"""Render original, editable README illustrations with Tauri's SVG renderer.

These are brand/product illustrations, not screenshots of an installed app.
Requires npm dependencies and Python Pillow; no downloaded game art is used.
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


def cover(x, y, palette, variant=0, scale=1):
    sky, accent, rock, shade = palette
    shapes = [rect(0, 0, 160, 240, sky, 5)]
    shapes.append(f'<circle cx="{96 if variant % 2 else 60}" cy="{67 if variant % 3 else 88}" r="{29 if variant % 2 else 42}" fill="{accent}"/>')
    if variant % 3 == 1:
        for px, py, w in [(10, 95, 26), (44, 121, 27), (77, 87, 22), (108, 108, 20), (136, 77, 17)]:
            shapes.append(rect(px, py, w, 240 - py, rock))
            for window_y in range(py + 12, 213, 20):
                shapes.append(rect(px + 7, window_y, 3, 5, accent))
    else:
        shapes.append(f'<path d="M0 173L37 103L62 139L101 83L160 181V240H0Z" fill="{rock}"/>')
        shapes.append(f'<path d="M0 197L46 145L91 187L132 129L160 168V240H0Z" fill="{shade}"/>')
        shapes.append(f'<path d="M49 240L79 162L88 162L73 240Z" fill="{accent}" opacity=".4"/>')
    shapes.append(f'<path d="M0 222L160 199V240H0Z" fill="{shade}"/>')
    return f'<g transform="translate({x} {y}) scale({scale})">' + ''.join(shapes) + '</g>'


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

    palettes = [('#1b2730', '#efc476', '#3b4954', '#13191e'),
                ('#192635', '#aacde1', '#304659', '#15212e'),
                ('#321f28', '#f38a64', '#513240', '#20151e'),
                ('#21282b', '#bdcc96', '#404c43', '#17201d'),
                ('#29223a', '#c5b1eb', '#4f416a', '#211a2e'),
                ('#33282c', '#e7b377', '#5d4040', '#251d22'),
                ('#152c29', '#9acbb4', '#345348', '#12221e')]
    names = ['Solstice', 'Northbound', 'Iron Harbor', 'Meridian', 'Aster', 'Afterlight', 'Verdant']
    scores = [(84, '7.4', 87), (92, '8.5', 94), (78, '7.8', 89), (73, '6.5', 81), (88, '8.1', 91), (61, '4.8', 68), (90, '8.6', 96)]
    ui = rect(0, 0, 1440, 940, '#0c0d10')
    ui += rect(32, 28, 1376, 872, '#101012', 12, '#303036')
    ui += embed('arc-logo.svg', 76, 45, 116, 38)
    ui += text(236, 70, 'Library', 15, weight=600)
    ui += text(326, 70, 'Recently played', 15, '#a5a5ae')
    ui += text(480, 70, 'Favorites', 15, '#a5a5ae')
    ui += '<path d="M236 89H286" stroke="#ece7de" stroke-width="2"/>'
    ui += rect(1053, 46, 274, 36, '#1b1b1f', 7)
    ui += text(1070, 70, 'Search games', 13, '#a5a5ae')
    ui += text(1290, 69, 'Ctrl K', 10, '#a5a5ae')
    ui += rect(32, 102, 1376, 354, '#19232b')
    ui += '<circle cx="1133" cy="223" r="88" fill="#e9bd72"/>'
    ui += '<path d="M570 456L802 244L920 327L1102 173L1408 400V456Z" fill="#3c4b55"/>'
    ui += '<path d="M559 456L808 325L966 398L1210 263L1408 389V456Z" fill="#202e38"/>'
    ui += '<path d="M978 456L1070 322H1083L1030 456Z" fill="#b99861"/>'
    ui += '<defs><linearGradient id="shade"><stop stop-color="#101012" stop-opacity=".96"/><stop offset=".76" stop-color="#101012" stop-opacity=".08"/></linearGradient></defs>'
    ui += rect(32, 102, 1376, 354, 'url(#shade)')
    ui += text(82, 171, 'ACTION ADVENTURE', 12, '#b8b8c1', 500, 2)
    ui += text(78, 239, 'SOLSTICE', 54, weight=650, spacing=-1)
    ui += text(82, 284, 'Follow the light beyond the last horizon.', 16, '#c6c6cf')
    ui += rect(82, 312, 112, 42, '#ece7de', 6)
    ui += '<path d="M101 325L101 341L114 333Z" fill="#181716"/>'
    ui += text(125, 339, 'Play', 15, '#181716', 600)
    ui += text(210, 339, 'Last played yesterday', 13, '#b7b7c0')
    for index, palette in enumerate(palettes[:4]):
        ui += cover(1098 + index * 63, 350, palette, index, .31)
    ui += text(79, 510, 'Library', 28, weight=600, spacing=-.5)
    ui += text(1134, 510, 'All games', 13, '#a5a5ae')
    ui += text(1267, 510, 'A–Z', 13, '#a5a5ae')
    for index, (palette, name, rating) in enumerate(zip(palettes, names, scores)):
        x = 80 + index * 188
        ui += cover(x, 543, palette, index)
        ui += text(x, 812, name, 14, '#dedee4', 500)
        critic, user, steam = rating
        user_color = '#8ee5ac' if float(user) >= 7.5 else '#ffd478' if float(user) >= 5 else '#ff9792'
        critic_color = '#8ee5ac' if critic >= 75 else '#ffd478'
        steam_color = '#8ee5ac' if steam >= 75 else '#ffd478'
        ui += f'<text x="{x}" y="837" font-size="11" fill="#a5a5ae">MC <tspan fill="{critic_color}">{critic}</tspan>/<tspan fill="{user_color}">{user}</tspan> · Steam <tspan fill="{steam_color}">{steam}</tspan></text>'
    ui += '<path d="M80 862H1360" stroke="#303036"/>'
    ui += text(82, 887, '7 sample games', 11, '#a5a5ae')
    ui += text(1226, 887, 'Arc 0.1.8', 11, '#a5a5ae')
    ui += text(35, 929, 'Illustrated UI preview · original sample artwork and scores', 14, '#a5a5ae')
    render('library-illustration', document(1440, 940, ui), 1440, 940)
    print('Rendered hero and clearly labeled library illustration; original vector sources retained.')


if __name__ == '__main__':
    main()
