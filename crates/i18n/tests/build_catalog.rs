use craft_i18n::build_catalog::read;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Checkout(PathBuf);
impl Checkout {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("craft-catalog-build-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, locale: &str, app: &str, text: &str) {
        let directory = self.0.join("locales").join(locale);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(format!("{app}.tsv")), text).unwrap();
    }
}
impl Drop for Checkout {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn build_catalog_uses_app_then_shared_then_regional_parent() {
    let input = Checkout::new();
    input.put("pt", "common", "\tOpen\tAbrir\n\tClose\tFechar\n");
    input.put("pt", "example", "\tSave\tGuardar\n\tOpen\tAbrir documento\n");
    input.put("pt-br", "common", "\tOpen\tAbrir arquivo\n");
    input.put("pt-br", "example", "\tSave\tSalvar\n");
    let rows = read(&input.0, "example", "pt-br").unwrap().unwrap();
    let get = |source: &str| rows.iter().find(|row| row.context.is_empty() && row.source == source).unwrap().translation.as_str();
    assert_eq!(get("Open"), "Abrir arquivo");
    assert_eq!(get("Close"), "Fechar");
    assert_eq!(get("Save"), "Salvar");
}

#[test]
fn build_catalog_preserves_context_escapes_and_rust_format_specifiers() {
    let input = Checkout::new();
    input.put("fr", "example", "smart album value\tNo\tNon\npreview\tDepth {z:.1}\\n\tProfondeur {z:.1}\\n\n");
    let rows = read(&input.0, "example", "fr").unwrap().unwrap();
    assert!(rows.iter().any(|row| row.context == "smart album value" && row.source == "No" && row.translation == "Non"));
    assert!(rows.iter().any(|row| row.context == "preview" && row.source == "Depth {z:.1}\n" && row.translation == "Profondeur {z:.1}\n"));
}

#[test]
fn build_catalog_missing_app_does_not_offer_shared_only_translation() {
    let input = Checkout::new();
    input.put("fr", "common", "\tOpen\tOuvrir\n");
    assert!(read(&input.0, "example", "fr").unwrap().is_none());
}

#[test]
fn build_catalog_rejects_bad_rows_and_path_components() {
    let input = Checkout::new();
    input.put("fr", "example", "\tOpen {name}\tOuvrir\n");
    assert!(read(&input.0, "example", "fr").unwrap_err().contains("placeholders"));
    for (app, locale) in [("../example", "fr"), ("example", "../fr"), ("", "fr"), ("example", "fr/extra")] {
        assert!(read(&input.0, app, locale).is_err());
    }
}

#[test]
fn build_catalog_script_locales_do_not_fall_back_to_another_script() {
    let input = Checkout::new();
    input.put("sr", "example", "\tOpen\tОтвори\n");
    input.put("sr-latn", "example", "\tSave\tSačuvaj\n");
    let rows = read(&input.0, "example", "sr-latn").unwrap().unwrap();
    assert!(rows.iter().all(|row| row.source != "Open"));
}

#[test]
fn build_catalog_refuses_oversized_input_before_utf8_decoding() {
    let input = Checkout::new();
    let mut text = "#".repeat(16 * 1024 * 1024);
    text.push('é');
    input.put("fr", "example", &text);
    assert!(read(&input.0, "example", "fr").unwrap_err().contains("catalog limit"));
}
