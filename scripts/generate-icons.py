#!/usr/bin/env python3
"""Regenerate native icons from the source SVG; requires rsvg-convert."""
from pathlib import Path
import struct
import subprocess

root = Path(__file__).resolve().parents[1] / 'packaging'
def png(size):
    return subprocess.check_output(['rsvg-convert', '-w', str(size), '-h', str(size), str(root / 'g502x-control.svg')])
image = png(256)
(root / 'g502x-control.png').write_bytes(png(512))
(root / 'g502x-control.ico').write_bytes(struct.pack('<HHH', 0, 1, 1) + struct.pack('<BBBBHHII', 0, 0, 0, 0, 1, 32, len(image), 22) + image)
chunks = b''
for kind, size in [(b'ic07',128), (b'ic08',256), (b'ic09',512), (b'ic10',1024)]:
    data = png(size)
    chunks += kind + struct.pack('>I', len(data)+8) + data
(root / 'g502x-control.icns').write_bytes(b'icns' + struct.pack('>I',len(chunks)+8) + chunks)
