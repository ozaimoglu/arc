"""Render the editable Eclipse README brand poster with Tauri's SVG renderer.

Requires npm dependencies and Python Pillow. Application screenshots must be
captured from Arc itself; this script only generates the brand poster.
"""
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/assets'
NS = 'http://www.w3.org/2000/svg'
ET.register_namespace('', NS)


def embed(name, x, y, width, height):
    svg = ET.parse(ROOT / 'public' / name).getroot()
    svg.attrib.update(x=str(x), y=str(y), width=str(width), height=str(height))
    return ET.tostring(svg, encoding='unicode')


def text(x, y, value, size=20, color='#ece7de', weight=400, spacing=0):
    return f'<text x="{x}" y="{y}" fill="{color}" font-size="{size}" font-weight="{weight}" letter-spacing="{spacing}">{value}</text>'


def rect(x, y, width, height, fill, radius=0, stroke='none'):
    return f'<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}" fill="{fill}" stroke="{stroke}"/>'


def document(width, height, body):
    return f'<svg xmlns="{NS}" viewBox="0 0 {width} {height}" font-family="Segoe UI, Arial, sans-serif">{body}</svg>'


def render(name, svg, width, height):
    source = OUT / f'{name}.svg'
    source.write_text(svg, encoding='utf-8')
    with tempfile.TemporaryDirectory(prefix='arc-readme-') as temporary:
        output = Path(temporary)
        subprocess.run(['node', str(ROOT / 'node_modules/@tauri-apps/cli/tauri.js'),
                        'icon', str(source), '--output', str(output), '--png', str(width),
                        '--fit', 'contain'], cwd=ROOT, check=True, capture_output=True)
        with Image.open(output / f'{width}x{width}.png') as image:
            top = (width - height) // 2
            image.crop((0, top, width, top + height)).convert('RGB').save(OUT / f'{name}.png', optimize=True)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    hero = rect(0, 0, 1600, 840, '#101012')
    hero += embed('arc-logo.svg', 84, 45, 250, 82)
    hero += text(1194, 92, 'WINDOWS X64 / TAURI 2', 18, '#a5a5ae', 500, 2)
    hero += text(92, 274, 'Your games.', 96, weight=650, spacing=-4)
    hero += text(92, 376, 'One cinematic', 96, weight=650, spacing=-4)
    hero += text(92, 478, 'library.', 96, '#ffd08b', 650, -4)
    hero += text(96, 556, 'Discover your installations. Curate the artwork.', 26, '#b9b9c1')
    hero += text(96, 598, 'Bring PC and shadPS4 games into one local library.', 26, '#b9b9c1')
    hero += embed('arc.svg', 1025, 162, 458, 458)
    hero += '<path d="M92 697H1508" stroke="#303036"/>'
    for x, title, subtitle in [(92, 'Local library', 'SQLite + offline cache'), (455, 'Artwork you choose', 'Covers, heroes and logos'), (825, 'Scores at a glance', 'Metacritic + Steam'), (1190, 'Open to build on', 'Rust + React + MIT')]:
        hero += text(x, 759, title, 24, weight=600)
        hero += text(x, 801, subtitle, 18, '#a5a5ae')
    render('hero', document(1600, 840, hero), 1600, 840)

    print('Rendered Eclipse brand poster; editable vector source retained.')


if __name__ == '__main__':
    main()
