#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// A ToUnicode CMap with a token the scanner can't read before its mappings (a stray `>` after
/// a CMap name that holds one, as in pdf.js `issue11651.pdf`; a CMap name with spaces in it, as
/// in `issue7492.pdf`) still maps its codes (vendored hayro-cmap patch 3, hayro-postscript
/// patch 2): the scan stopped at that token, so the page's text could not be copied or found.
#[test]
fn a_to_unicode_cmap_reads_its_mappings_past_a_token_it_cannot_parse() {
    use hayro_cmap::{BfString, CMap};
    let body = "1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n2 beginbfchar\n<002B> <0048>\n<0048> <0065>\nendbfchar\nendcmap\n";
    let stray = format!(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CIDSystemInfo <</Registry (Adobe) /Ordering (UC>) /Supplement 0>> def\n/CMapName /Adobe-Identity-UC> def\n/CMapType 2 def\n{body}"
    );
    let spaced = format!(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CMapName /-usr-share-fonts-Panton Family-Fontfabric - Panton.otf,000-UTF16 def\n/CMapType 2 def\n{body}"
    );
    for (what, data) in [("a stray >", stray), ("a name with spaces", spaced)] {
        let cmap = CMap::parse(data.as_bytes(), |_| None).unwrap_or_else(|| panic!("{what}: parsed"));
        assert_eq!(cmap.lookup_bf_string(0x2B), Some(BfString::Char('H')), "{what}");
        assert_eq!(cmap.lookup_bf_string(0x48), Some(BfString::Char('e')), "{what}");
    }
    // Unreadable tokens are stepped over a bounded number of times: a long run of them ends
    // the scan rather than costing its length squared, and what came before is kept.
    let flood = format!("{body}{}", "-".repeat(1 << 16));
    assert!(CMap::parse(flood.as_bytes(), |_| None).is_some_and(|c| c.lookup_bf_string(0x2B).is_some()));
}
