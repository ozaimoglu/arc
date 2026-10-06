"""Copy only cached npm dependencies into the workspace; never writes to the source cache."""
import base64
import hashlib
import json
import re
import shutil
import tarfile
from pathlib import Path

source = Path.home() / 'AppData/Local/npm-cache/_cacache'
destination = Path(__file__).resolve().parents[1] / '.cache/npm/_cacache'
entries = {}
for path in (source / 'index-v5').rglob('*'):
    if path.is_file():
        for line in path.read_text(errors='ignore').splitlines():
            try:
                value = json.loads(line.split('\t', 1)[1])
                if value.get('integrity'):
                    entries[value['key']] = (path, value)
            except (ValueError, IndexError):
                pass

def content(value):
    algorithm, digest = value['integrity'].split(' ', 1)[0].split('-', 1)
    hex_digest = base64.b64decode(digest).hex()
    return Path('content-v2') / algorithm / hex_digest[:2] / hex_digest[2:4] / hex_digest[4:]

manifest = json.loads((destination.parents[2] / 'package.json').read_text())
needed = set(manifest['dependencies']) | set(manifest['devDependencies'])
visited = set()
copied = 0
while needed - visited:
    name = next(iter(needed - visited))
    visited.add(name)
    matched = []
    prefix = 'make-fetch-happen:request-cache:https://registry.npmjs.org/'
    for key, (index, value) in entries.items():
        suffix = key.removeprefix(prefix)
        if key.startswith(prefix) and (suffix.startswith(name + '/-/') or suffix in (name, name.replace('/', '%2f'))):
            matched.append((index, value))
    for index, value in matched:
        relative = content(value)
        original = source / relative
        if not original.is_file():
            continue
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            shutil.copyfile(original, target)
            copied += 1
        index_target = destination / index.relative_to(source)
        index_target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(index, index_target)
        if '/-/' in value['key']:
            try:
                with tarfile.open(original, 'r:gz') as archive:
                    member = next((entry for entry in archive.getmembers() if entry.name.endswith('/package.json') and entry.name.count('/') == 1), None)
                    file = archive.extractfile(member) if member else None
                    if file:
                        package = json.load(file)
                        needed.update(package.get('dependencies', {}))
                        needed.update(package.get('optionalDependencies', {}))
            except (tarfile.TarError, ValueError, KeyError):
                pass
print(f'Imported {copied} cached entries across {len(visited)} dependency names.')
