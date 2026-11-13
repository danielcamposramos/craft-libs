//! Build-time access to the same catalog rows and locale precedence as the runtime lookup.
//! Apps keep their own language registry and presentation format, including checked Rust
//! format macros. The data checkout is a build input, never a Cargo dependency.

use std::{collections::BTreeMap, fs, io::Read, path::Path};

/// One validated, unescaped row, after app/shared and regional precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub context: String,
    pub source: String,
    pub translation: String,
}

/// Read an app's rows from an optional data checkout already selected by its build script.
/// `None` means this locale has no app catalog; shared-only locales are not offered.
/// Invalid rows, unreadable inputs and path components return an explicit error.
pub fn read(checkout: &Path, app: &str, locale: &str) -> Result<Option<Vec<Row>>, String> {
    let component = |value: &str| {
        !value.is_empty() && value.split('-').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
    };
    if !component(app) || !component(locale) {
        return Err("catalog app and locale must be lowercase BCP-47-like path components".into());
    }
    let directory = checkout.join("locales");
    if !directory.is_dir() {
        return Err(format!("{} is not a catalog directory", directory.display()));
    }
    let Some(app_text) = text(&directory.join(locale).join(format!("{app}.tsv")))? else { return Ok(None) };
    let parent = locale.rsplit_once('-').filter(|(_, region)| crate::is_region(region)).map(|(parent, _)| parent);
    let mut rows = BTreeMap::new();
    if let Some(parent) = parent
        && let Some(parent_app) = text(&directory.join(parent).join(format!("{app}.tsv")))?
    {
        merge(&mut rows, &directory, app, parent, &parent_app)?;
    }
    merge(&mut rows, &directory, app, locale, &app_text)?;
    Ok(Some(rows.into_values().collect()))
}

fn text(path: &Path) -> Result<Option<String>, String> {
    // These are source catalogs, not arbitrary documents. Bound an invalid build input before
    // allocating it; ordinary suite catalogs are well below this limit.
    const MAX_BYTES: usize = 16 * 1024 * 1024;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() > MAX_BYTES {
        return Err(format!("{} exceeds the {MAX_BYTES}-byte catalog limit", path.display()));
    }
    let value = String::from_utf8(bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Some(value))
}

fn merge(rows: &mut BTreeMap<(String, String), Row>, directory: &Path, app: &str, locale: &str, app_text: &str) -> Result<(), String> {
    let shared = text(&directory.join(locale).join("common.tsv"))?.unwrap_or_default();
    let forms = crate::plural_rules(locale).as_ref().map_or(1, |(_, categories)| categories.len());
    for (name, source) in [("common", shared.as_str()), (app, app_text)] {
        let (parsed, problems) = crate::catalog::parse(source, forms);
        if !problems.is_empty() {
            return Err(format!("{locale}/{name}: {}", problems.join("; ")));
        }
        for row in parsed {
            rows.insert((row.context.clone(), row.source.clone()), Row { context: row.context, source: row.source, translation: row.translation });
        }
    }
    Ok(())
}
