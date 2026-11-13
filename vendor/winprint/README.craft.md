# Pinned Windows print wrapper

The BSD-3-Clause package source comes from [Storytold winprint.rs e90c444](https://github.com/storytold/winprint.rs/tree/e90c444476609485ea3f5ff67ab12f65d75b11df).
`SOURCE.json` records the original file hashes and omissions.
The independent `craft-winprint-gdi` dependency remains at its existing public revision; this snapshot does not replace that public Rust type identity.

The next carried commit is dotCooCoo's [PrintTicket::from_dev_mode, winprint.rs#24](https://github.com/storytold/winprint.rs/pull/24), revision `72951d7455ebcab569fbd9386c513d3d9f53fb02`.
Its production patch is retained unchanged, under its original authorship and date.
The snapshot makes this pending fix available without relaxing a receiving app's rule against Git dependencies outside Storytold.
It does not claim that the pending fix has merged.

The six upstream binary test fixtures are replaced with original font-free A4 patterns, licensed CC0-1.0 with individual sidecars and hashes in `FIXTURES.json`.
Generate them with `python3 make_fixtures.py`.
The original PDF fixtures embedded fonts whose redistribution terms were not established for this import.
No font, third-party document or image is imported from those fixtures.
The Rust test sources and the two XML fixtures are unchanged, apart from default-rustfmt layout in `src/ticket/document/writer.rs`.
`src/printer/dxgi.rs` also has only a default-rustfmt layout adjustment; both exceptions are hashed separately in `SOURCE.json`.
The carried `PrintTicket::from_dev_mode` file remains byte-identical to its upstream fix.
The PDF and TIFF representations are independently compared after PDF rasterization; Windows printing and XPS rendering still require Windows.
Upstream's separate GDI crate, workflows, editor configuration and fixture-generation utility are omitted because they are not part of this Cargo package's source dependency.

Keep `default-features = false` in the consumer: the optional `pdfium` feature retains upstream's download behavior and is not enabled by this import.
The source and fixture migration used AI assistance; the import audit and automated checks are recorded with this change.
