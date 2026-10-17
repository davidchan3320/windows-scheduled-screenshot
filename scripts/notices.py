"""Refresh embedded dependency license notices from `cargo metadata` output.
Run cargo fetch first. Includes dev/build packages as a conservative superset.
"""
import json
import hashlib
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'], cwd=root))
included = {node['id'] for node in metadata['resolve']['nodes']}
parts = ['Scheduled Screenshot — third-party notices\n\nThe following third-party packages are used by the application or its build/test tooling.\n']
texts = {}
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if not package['source'] or package['id'] not in included:
        continue
    directory = Path(package['manifest_path']).parent
    parts.append('\n' + '=' * 72 + '\n' + package['name'] + ' ' + package['version'] + '\nLicense: ' + (package['license'] or 'See included license') + '\n' + (package['repository'] or '') + '\n')
    files = [p for p in directory.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT'))]
    for file in sorted(files):
        text = file.read_text(encoding='utf-8', errors='replace')
        key = hashlib.sha256(text.encode()).hexdigest()[:12]
        texts[key] = text
        parts.append(file.name + ': see license text ' + key + '\n')
    if not files:
        raise RuntimeError('No license text found for ' + package['name'] + '; inspect package manually.')
# Generated Tailwind CSS is distributed inside the executable. Keep its license
# in the repository so notice refreshes do not require the Node toolchain.
tailwind_license = (root / 'ui/tailwind-LICENSE.txt').read_text(encoding='utf-8')
tailwind_key = hashlib.sha256(tailwind_license.encode()).hexdigest()[:12]
texts[tailwind_key] = tailwind_license
parts.append('\n' + '=' * 72 + '\nTailwind CSS 4.3.3\nLicense: MIT\nhttps://github.com/tailwindlabs/tailwindcss\nLICENSE: see license text ' + tailwind_key + '\n')
for key, text in texts.items():
    parts.append('\n' + '=' * 72 + '\nLICENSE TEXT ' + key + '\n\n' + text + '\n')
(root / 'THIRD_PARTY_NOTICES.txt').write_text(''.join(parts), encoding='utf-8')
