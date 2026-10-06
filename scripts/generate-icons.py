"""Build Arc's logo and Windows icons from the shared vector brand sources.

Requires project npm dependencies and Pillow. Tauri's own SVG renderer keeps
the native icon, favicon and header mark consistent without duplicating paths.
"""
from copy import deepcopy
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
PUBLIC = ROOT / 'public'
OUT = ROOT / 'src-tauri/icons'
SVG_NS = 'http://www.w3.org/2000/svg'
ET.register_namespace('', SVG_NS)


def tag(name):
    return f'{{{SVG_NS}}}{name}'


def write_svg(element, path):
    ET.indent(element, space='  ')
    ET.ElementTree(element).write(path, encoding='utf-8', xml_declaration=False)


def render_png(source, output, size, fit=None):
    command = ['node', str(ROOT / 'node_modules/@tauri-apps/cli/tauri.js'),
               'icon', str(source), '--output', str(output), '--png', str(size)]
    if fit:
        command += ['--fit', fit]
    subprocess.run(command, cwd=ROOT, check=True, capture_output=True)
    return output / f'{size}x{size}.png'


def main():
    mark = ET.parse(PUBLIC / 'arc-mark.svg').getroot()
    wordmark = ET.parse(PUBLIC / 'arc-wordmark.svg').getroot()
    icon = ET.Element(tag('svg'), {'viewBox': '0 0 128 128', 'fill': 'none'})
    defs = deepcopy(mark.find(tag('defs')))
    tile_gradient = ET.SubElement(defs, tag('linearGradient'), {
        'id': 'arc-tile', 'x1': '16', 'y1': '4', 'x2': '112', 'y2': '128',
        'gradientUnits': 'userSpaceOnUse',
    })
    for offset, color in [('0', '#292a30'), ('.55', '#17191f'), ('1', '#0b0c10')]:
        ET.SubElement(tile_gradient, tag('stop'), {'offset': offset, 'stop-color': color})
    icon.append(defs)
    ET.SubElement(icon, tag('rect'), {
        'x': '2', 'y': '2', 'width': '124', 'height': '124', 'rx': '28',
        'fill': 'url(#arc-tile)', 'stroke': '#ffffff', 'stroke-opacity': '.12',
    })
    for child in mark:
        if child.tag != tag('defs'):
            icon.append(deepcopy(child))
    write_svg(icon, PUBLIC / 'arc.svg')

    logo = ET.Element(tag('svg'), {'viewBox': '0 0 392 128', 'fill': 'none'})
    for child in mark:
        logo.append(deepcopy(child))
    word_group = ET.SubElement(logo, tag('g'), {
        'transform': 'translate(150 28)', 'fill': wordmark.get('fill'),
    })
    for child in wordmark:
        word_group.append(deepcopy(child))
    write_svg(logo, PUBLIC / 'arc-logo.svg')

    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='arc-brand-') as work:
        work = Path(work)
        icon_source = render_png(PUBLIC / 'arc.svg', work / 'icon', 1024)
        with Image.open(icon_source) as image:
            image.convert('RGBA').save(PUBLIC / 'arc-icon.png')
            image.convert('RGBA').resize((256, 256), Image.Resampling.LANCZOS).save(OUT / 'icon.png')
            image.convert('RGBA').save(OUT / 'icon.ico', sizes=[
                (16, 16), (24, 24), (32, 32), (48, 48),
                (64, 64), (128, 128), (256, 256),
            ])
        logo_source = render_png(PUBLIC / 'arc-logo.svg', work / 'logo', 1536, 'contain')
        with Image.open(logo_source) as image:
            # The SVG is contained in a square canvas; crop only its letterbox.
            logo_height = round(1536 * 128 / 392)
            top = (1536 - logo_height) // 2
            image.crop((0, top, 1536, top + logo_height)).save(PUBLIC / 'arc-logo.png')
    print('Generated shared Arc logo, favicon and seven-size Windows ICO.')


if __name__ == '__main__':
    main()
