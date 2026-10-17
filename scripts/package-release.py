"""Package the Docker-built executable with a ZIP and SHA-256 checksums."""
import hashlib
from pathlib import Path
import sys
import xml.etree.ElementTree as ET
import zipfile

import pefile

release = Path(sys.argv[1])
exe = release / 'ScheduledScreenshot.exe'
archive = release / 'ScheduledScreenshot-win-x64.zip'

# Check the linked resources, not just the source manifest: LLVM's manifest
# merger can change XML namespaces while still returning a successful build.
pe = pefile.PE(str(exe))
if pe.FILE_HEADER.Machine != 0x8664 or pe.OPTIONAL_HEADER.Subsystem != 2:
    raise ValueError('Expected a Windows x64 GUI executable.')
manifests = []
for kind in pe.DIRECTORY_ENTRY_RESOURCE.entries:
    if kind.id == 24:  # RT_MANIFEST
        for resource in kind.directory.entries:
            for language in resource.directory.entries:
                data = language.data.struct
                manifests.append(ET.fromstring(pe.get_data(data.OffsetToData, data.Size)))
if len(manifests) != 1:
    raise ValueError('Expected one embedded application manifest.')
manifest = manifests[0]
level = manifest.find('.//{urn:schemas-microsoft-com:asm.v3}requestedExecutionLevel')
if level is None or level.get('level') != 'asInvoker' or level.get('uiAccess') != 'false':
    raise ValueError('Embedded manifest has an invalid UAC declaration.')
for name, expected in [('dpiAwareness', 'PerMonitorV2'), ('longPathAware', 'true')]:
    setting = manifest.find('.//{http://schemas.microsoft.com/SMI/2016/WindowsSettings}' + name)
    if setting is None or setting.text != expected:
        raise ValueError(f'Embedded manifest has an invalid {name} setting.')
for entry in pe.DIRECTORY_ENTRY_IMPORT:
    if entry.dll.lower().startswith((b'vcruntime', b'msvcp', b'ucrtbase', b'api-ms-win-crt')):
        raise ValueError(f'Unexpected dynamic CRT dependency: {entry.dll.decode()}')
pe.close()

with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as package:
    package.write(exe, arcname=exe.name)
(release / 'SHA256SUMS.txt').write_text(
    ''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n'
            for path in (exe, archive)),
    encoding='ascii',
)
