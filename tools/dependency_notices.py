"""Collect existing local dependency texts, not license clearance or linker proof."""
import json
import os
from pathlib import Path
import re
import stat


def public_source(source):
    # Other registries/git URLs may name private hosts, paths or credentials.
    if source is None:
        return 'workspace-or-path-source'
    if source in ('registry+https://github.com/rust-lang/crates.io-index',
                  'sparse+https://index.crates.io/'):
        return source
    return 'redacted-unverified-source'


def collect(metadata, allowed_roots):
    roots = [Path(p).resolve(strict=True) for p in allowed_roots]
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    pending = [metadata['resolve']['root']]
    seen = set()
    while pending:
        key = pending.pop()
        if key in seen:
            continue
        seen.add(key)
        for dep in nodes[key]['deps']:
            if any(k['kind'] != 'dev' for k in dep['dep_kinds']):
                pending.append(dep['pkg'])
    files, rows = {}, []
    for key in sorted(seen):
        pkg = packages[key]
        name, version = pkg['name'], pkg['version']
        if not re.fullmatch(r'[A-Za-z0-9_-]+', name) or not re.fullmatch(r'[A-Za-z0-9.+_-]+', version):
            raise ValueError('Unsafe package identity')
        home = Path(pkg['manifest_path']).parent.resolve(strict=True)
        if not any(home.is_relative_to(root) for root in roots):
            raise ValueError('Dependency is outside explicitly allowed source roots')
        paths = set()
        # Crate archives also use lowercase names (notably windows-rs).
        # Inspect direct children only; never recurse or widen source roots.
        for child in home.iterdir():
            if child.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE')):
                paths.add(child)
        if pkg.get('license_file'):
            relative = Path(pkg['license_file'])
            if relative.is_absolute() or '..' in relative.parts:
                raise ValueError('License path escapes package')
            paths.add(home / relative)
        texts = []
        for path in sorted(paths):
            # Directories are not recursively searched. Symlinks are never followed.
            if path.is_symlink():
                raise ValueError('License symlink is not an explicit text input')
            if path.is_dir():
                continue
            relative = path.relative_to(home)
            current = home
            for part in relative.parts:
                current = current / part
                if current.is_symlink():
                    raise ValueError('License path contains a symlink')
            info = path.stat()
            if not stat.S_ISREG(info.st_mode) or info.st_size > 1024 * 1024:
                raise ValueError('Expected bounded regular license text')
            flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0)
            with os.fdopen(os.open(path, flags), 'rb') as stream:
                before = os.fstat(stream.fileno())
                data = stream.read(1024 * 1024 + 1)
                after = os.fstat(stream.fileno())
            if (not stat.S_ISREG(before.st_mode) or len(data) > 1024 * 1024
                    or (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
                    != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
                    or len(data) != before.st_size):
                raise ValueError('License text changed or exceeded the read bound')
            data.decode('utf-8')
            dest = f'dependency-notices/{name}-{version}/{relative.as_posix()}'
            if dest in files and files[dest] != data:
                raise ValueError('Conflicting license text for package identity')
            files[dest] = data
            texts.append(dest)
        rows.append({'name': name, 'version': version, 'source': public_source(pkg.get('source')),
                     'license': pkg.get('license'), 'texts': texts,
                     'status': 'texts-collected-review-required' if texts else 'missing-text-review-required'})
    files['dependency-notices/index.json'] = (json.dumps({
        'scope': 'normal/build metadata reachability, not final linked inclusion or legal clearance',
        'packages': rows}, indent=2) + '\n').encode()
    return files
