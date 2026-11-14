//! Unlinked incremental-update repair keeps the document's decoder policy.
#![cfg(feature = "read")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use craft_pdf::syntax::{
    DecodeLimits, Pdf,
    object::{ObjectIdentifier, Stream},
};

fn incremental(linked: bool) -> Vec<u8> {
    let objects: Vec<&[u8]> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Contents 4 0 R >>",
        b"<< /Length 3 /Filter /RunLengthDecode >> stream\n\x81X\x80\nendstream",
    ];
    let mut data = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (i, body) in objects.iter().enumerate() {
        offsets.push(data.len());
        data.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        data.extend_from_slice(body);
        data.extend_from_slice(b"\nendobj\n");
    }
    let base = data.len();
    data.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
    for offset in &offsets[1..] {
        data.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    data.extend_from_slice(format!("trailer << /Size 5 /Root 1 0 R >>\nstartxref\n{base}\n%%EOF\n").as_bytes());
    let updated = data.len();
    data.extend_from_slice(b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n");
    let latest = data.len();
    let previous = if linked { format!(" /Prev {base}") } else { String::new() };
    data.extend_from_slice(
        format!("xref\n1 1\n{updated:010} 00000 n \ntrailer << /Size 5 /Root 1 0 R{previous} >>\nstartxref\n{latest}\n%%EOF\n").as_bytes(),
    );
    data
}

#[test]
fn an_unlinked_update_is_reported_and_repair_keeps_the_document_decode_limit() {
    for linked in [false, true] {
        let limits = DecodeLimits::default().with_max_decoded_stream(16);
        let doc = Pdf::new_with_limits(incremental(linked), limits).unwrap();
        assert_eq!(doc.rebuilt_unlinked_update(), !linked);
        assert_eq!(doc.pages().len(), 1);
        assert_eq!(doc.decode_limits(), limits);
        let stream = doc.xref().get::<Stream<'_>>(ObjectIdentifier::new(4, 0)).unwrap();
        assert_eq!(stream.decoded().unwrap().len(), 16, "repair must retain the document limit; RunLength returns its bounded prefix");
        let ordinary = Pdf::new(incremental(linked)).unwrap();
        assert_eq!(ordinary.xref().get::<Stream<'_>>(ObjectIdentifier::new(4, 0)).unwrap().decoded().unwrap().len(), 128);
    }
}
