#!/usr/bin/env python3
"""Rasterize the original mouse SVG for the native UI. Requires rsvg-convert."""
from pathlib import Path
import subprocess

assets = Path(__file__).resolve().parents[1] / 'assets'
subprocess.run(['rsvg-convert', '-o', str(assets/'mouse.png'), str(assets/'mouse.svg')], check=True)
