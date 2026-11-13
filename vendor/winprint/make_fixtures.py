"""Original, font-free A4 patterns for the vendored printing tests.

SPDX-FileCopyrightText: 2026 Daniel Campos Ramos
SPDX-License-Identifier: CC0-1.0
"""
from pathlib import Path
import hashlib
import json
import struct
import zipfile
import zlib

HERE = Path(__file__).resolve().parent
WIDTH, HEIGHT = 2480, 3508
# Fractional page coordinates measured from the top left; no text or fonts.
BOXES = [(.12, .12, .76, .06, (24, 48, 72)),
         (.12, .25, .32, .25, (176, 40, 32)),
         (.56, .25, .32, .25, (24, 120, 80)),
         (.12, .58, .76, .20, (32, 80, 176)),
         (.36, .83, .28, .05, (40, 40, 40))]


def pdf():
    w, h = 210 / 25.4 * 72, 297 / 25.4 * 72
    commands = []
    for x, y, dx, dy, color in BOXES:
        commands.append('%s rg %.6f %.6f %.6f %.6f re f' % (
            ' '.join('%.6f' % (c / 255) for c in color),
            x * w, (1 - y - dy) * h, dx * w, dy * h))
    stream = ('\n'.join(commands) + '\n').encode()
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>',
               b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
               ('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 %.6f %.6f] '
                '/Resources << >> /Contents 4 0 R >>' % (w, h)).encode(),
               b'<< /Length %d >>\nstream\n' % len(stream) + stream + b'endstream']
    result = bytearray(b'%PDF-1.4\n%\xe2\xe3\xcf\xd3\n')
    offsets = [0]
    for number, value in enumerate(objects, 1):
        offsets.append(len(result))
        result.extend(b'%d 0 obj\n' % number + value + b'\nendobj\n')
    xref = len(result)
    result.extend(b'xref\n0 5\n0000000000 65535 f \n')
    for offset in offsets[1:]:
        result.extend(b'%010d 00000 n \n' % offset)
    result.extend(b'trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n' % xref)
    return bytes(result)


def tiff():
    compressor = zlib.compressobj(9)
    compressed = bytearray()
    for row in range(HEIGHT):
        pixels = bytearray(b'\xff' * WIDTH * 3)
        for x, y, dx, dy, color in BOXES:
            if round(y * HEIGHT) <= row < round((y + dy) * HEIGHT):
                a, b = round(x * WIDTH), round((x + dx) * WIDTH)
                pixels[a * 3:b * 3] = bytes(color) * (b - a)
        compressed.extend(compressor.compress(pixels))
    compressed.extend(compressor.flush())
    count = 12
    extra = 8 + 2 + count * 12 + 4
    strip = extra + 6 + 8 + 8
    entries = [(256, 4, 1, WIDTH), (257, 4, 1, HEIGHT), (258, 3, 3, extra),
               (259, 3, 1, 8), (262, 3, 1, 2), (273, 4, 1, strip),
               (277, 3, 1, 3), (278, 4, 1, HEIGHT),
               (279, 4, 1, len(compressed)), (282, 5, 1, extra + 6),
               (283, 5, 1, extra + 14), (296, 3, 1, 2)]
    out = bytearray(b'II' + struct.pack('<HIH', 42, 8, count))
    for tag, kind, n, value in entries:
        out.extend(struct.pack('<HHII', tag, kind, n, value))
    out.extend(struct.pack('<IHHHIIII', 0, 8, 8, 8, 300, 1, 300, 1))
    assert len(out) == strip
    return bytes(out + compressed)


def xps(path):
    ns = 'http://schemas.microsoft.com/xps/2005/06'
    w, h = 210 / 25.4 * 96, 297 / 25.4 * 96
    paths = []
    for x, y, dx, dy, color in BOXES:
        a, b, c, d = x * w, y * h, (x + dx) * w, (y + dy) * h
        paths.append('<Path Fill="#%02X%02X%02X" Data="M %.6f,%.6f L %.6f,%.6f %.6f,%.6f %.6f,%.6f Z"/>' % (*color, a, b, c, b, c, d, a, d))
    entries = {
        '[Content_Types].xml': '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        + ''.join('<Default Extension="%s" ContentType="application/vnd.ms-package.xps-%s+xml"/>' % p
                  for p in [('fdseq', 'fixeddocumentsequence'), ('fdoc', 'fixeddocument'), ('fpage', 'fixedpage')]) + '</Types>',
        '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="document" Type="%s/fixedrepresentation" Target="/FixedDocumentSequence.fdseq"/></Relationships>' % ns,
        'FixedDocumentSequence.fdseq': '<FixedDocumentSequence xmlns="%s"><DocumentReference Source="/Documents/1/FixedDocument.fdoc"/></FixedDocumentSequence>' % ns,
        'Documents/1/FixedDocument.fdoc': '<FixedDocument xmlns="%s"><PageContent Source="/Documents/1/Pages/1.fpage"/></FixedDocument>' % ns,
        'Documents/1/Pages/1.fpage': '<FixedPage xmlns="%s" xml:lang="en-US" Width="%.6f" Height="%.6f">%s</FixedPage>' % (ns, w, h, ''.join(paths)),
    }
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, text in sorted(entries.items()):
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, text.encode())


def main():
    rows = []
    for prefix in ['test_data/test_document', 'test_data/visual_regression/data']:
        stem = HERE / prefix
        stem.parent.mkdir(parents=True, exist_ok=True)
        stem.with_suffix('.pdf').write_bytes(pdf())
        stem.with_suffix('.tiff').write_bytes(tiff())
        xps(stem.with_suffix('.xps'))
        for suffix in ['.pdf', '.tiff', '.xps']:
            path = stem.with_suffix(suffix)
            path.with_suffix(suffix + '.license').write_text(
                'SPDX-FileCopyrightText: 2026 Daniel Campos Ramos\n'
                'SPDX-License-Identifier: CC0-1.0\n'
                'Original font-free geometric test pattern; generated by make_fixtures.py.\n')
            rows.append(dict(path=path.relative_to(HERE).as_posix(),
                             sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                             license='CC0-1.0'))
    (HERE / 'FIXTURES.json').write_text(json.dumps(rows, indent=2) + '\n')


if __name__ == '__main__':
    main()
