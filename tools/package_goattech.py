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

from dependency_notices import collect

ROOT = Path(__file__).resolve().parents[1]


def package(binary, output, platform, version, source_commit, dependency_metadata=None, dependency_roots=()):
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
    notices = {
        'THIRD_PARTY_NOTICES.md': 'THIRD_PARTY_NOTICES.md',
        'notices/OFL-Geist.txt': 'crates/horizon-theme/fonts/OFL-Geist.txt',
        'notices/OFL-BricolageGrotesque.txt': 'crates/horizon-theme/fonts/OFL-BricolageGrotesque.txt',
        'notices/OFL-PixelifySans.txt': 'crates/horizon-theme/fonts/OFL-PixelifySans.txt',
        'notices/LICENSE-lucide.txt': 'crates/horizon-theme/icons/lucide/LICENSE-lucide.txt',
        'notices/LICENSE-pixelarticons.txt': 'crates/horizon-theme/icons/pixel/LICENSE-pixelarticons.txt',
        'notices/naga-LICENSE.MIT': 'third_party/naga/LICENSE.MIT',
        'notices/naga-LICENSE.APACHE': 'third_party/naga/LICENSE.APACHE',
        'notices/wgpu-hal-LICENSE.MIT': 'third_party/wgpu-hal/LICENSE.MIT',
        'notices/wgpu-hal-LICENSE.APACHE': 'third_party/wgpu-hal/LICENSE.APACHE',
    }
    files.update({name: (ROOT / path).read_bytes() for name, path in notices.items()})
    # MPL-2.0 executable distribution: retain the exact unmodified covered
    # source archive and tell recipients how to obtain it locally.
    covered_source = ROOT / 'packaging/goattech/covered-source/option-ext-0.2.0.crate'
    source_bytes = covered_source.read_bytes()
    if hashlib.sha256(source_bytes).hexdigest() != '04744f49eae99ab78e0d5c0b603ab218f515ea8cfe5a456d7629ad883a3b6e7d':
        raise ValueError('Covered source archive does not match the locked crate checksum')
    files['covered-source/option-ext-0.2.0.crate'] = source_bytes
    files['covered-source/README.txt'] = (ROOT / 'packaging/goattech/covered-source/README.txt').read_bytes()

    if dependency_metadata is not None:
        files.update(collect(dependency_metadata, dependency_roots,
                             ROOT / 'packaging/goattech/upstream-notices'))
    manifest = {
        'product': 'GoatTech Launcher', 'version': version,
        'platform': platform, 'source_commit': source_commit,
        'kind': 'portable candidate, not publication approval',
        'dependency_notices': ('metadata texts collected; review still required'
                               if dependency_metadata is not None else 'not collected; review required'),
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
    parser.add_argument('--dependency-metadata', required=True)
    parser.add_argument('--dependency-root', action='append', required=True)
    args = parser.parse_args()
    metadata = json.loads(Path(args.dependency_metadata).read_text())
    print(package(args.binary, args.output, args.platform, args.version, args.source_commit,
                  metadata, args.dependency_root))


if __name__ == '__main__':
    main()
