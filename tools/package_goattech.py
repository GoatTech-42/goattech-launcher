#!/usr/bin/env python3
"""Make a build-only portable candidate from one explicit launcher binary.

This does not sign, publish, enable updates, or include any runtime profile.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import stat
import tarfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def package(binary, output, platform, version, source_commit):
    if platform not in ('windows-x64', 'linux-x64'):
        raise ValueError('Only Windows x64 and Linux x64 candidates are supported')
    if not re.fullmatch(r'0\.1\.[0-9]+(?:-rc\.[1-9][0-9]*)?', version):
        raise ValueError('Expected the independent GoatTech 0.1.x version line')
    if not re.fullmatch(r'[0-9a-f]{40}', source_commit):
        raise ValueError('Expected a full source commit SHA')
    binary, output = Path(binary), Path(output)
    info = binary.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise ValueError('The binary must be a regular, single-link file, not a symlink')
    data = binary.read_bytes()
    if not data.startswith(b'MZ' if platform == 'windows-x64' else b'\x7fELF'):
        raise ValueError('The binary does not match the selected platform')
    if not output.is_dir() or output.is_symlink():
        raise ValueError('Output must be an existing directory, not a symlink')
    suffix = '.zip' if platform == 'windows-x64' else '.tar.gz'
    archive = output / f'goattech-launcher-{version}-{platform}{suffix}'
    sums = output / 'SHA256SUMS'
    if archive.exists() or archive.is_symlink() or sums.exists() or sums.is_symlink():
        raise FileExistsError('Candidate outputs already exist; use a fresh directory')
    name = 'horizon-launcher.exe' if platform == 'windows-x64' else 'horizon-launcher'
    files = {
        name: data,
        'README.txt': (ROOT / 'packaging/goattech/README.txt').read_bytes(),
        'LICENSE': (ROOT / 'LICENSE').read_bytes(),
        'goattech.svg': (ROOT / 'crates/horizon-theme/icons/goattech.svg').read_bytes(),
    }
    manifest = {
        'product': 'GoatTech Launcher', 'version': version,
        'platform': platform, 'source_commit': source_commit,
        'kind': 'portable candidate, not publication approval',
        'sha256': {n: hashlib.sha256(b).hexdigest() for n, b in files.items()},
    }
    files['manifest.json'] = (json.dumps(manifest, indent=2) + '\n').encode()
    # Exclusive creation preserves any existing candidate, including a raced writer.
    with archive.open('xb') as raw:
        if platform == 'windows-x64':
            with zipfile.ZipFile(raw, 'w', compression=zipfile.ZIP_DEFLATED) as z:
                for n, b in files.items():
                    z.writestr(n, b)
        else:
            with tarfile.open(fileobj=raw, mode='w:gz') as t:
                for n, b in files.items():
                    entry = tarfile.TarInfo(n)
                    entry.mode = 0o755 if n == name else 0o644
                    entry.size = len(b)
                    t.addfile(entry, io.BytesIO(b))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    with sums.open('x') as f:
        f.write(f'{digest}  {archive.name}\n')
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('binary', 'output', 'platform', 'version', 'source-commit'):
        parser.add_argument('--' + flag, required=True)
    args = parser.parse_args()
    print(package(args.binary, args.output, args.platform, args.version, args.source_commit))


if __name__ == '__main__':
    main()
