"""Collect existing local dependency texts, not license clearance or linker proof."""
import json
import hashlib
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


def read_text(path):
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
    return data


def collect(metadata, allowed_roots, supplemental_root=None):
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
    supplemental = {}
    if supplemental_root is not None:
        supplemental_root = Path(supplemental_root)
        if supplemental_root.is_symlink():
            raise ValueError('Supplemental root must not be a symlink')
        supplemental_root = supplemental_root.resolve(strict=True)
        if (supplemental_root / 'index.json').is_symlink():
            raise ValueError('Supplemental index must not be a symlink')
        index = json.loads(read_text(supplemental_root / 'index.json'))
        for row in index['packages']:
            identity = (row['name'], row['version'])
            if identity in supplemental:
                raise ValueError('Duplicate supplemental identity')
            supplemental[identity] = row
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
            data = read_text(path)
            dest = f'dependency-notices/{name}-{version}/{relative.as_posix()}'
            if dest in files and files[dest] != data:
                raise ValueError('Conflicting license text for package identity')
            files[dest] = data
            texts.append(dest)
        supplement = supplemental.get((name, version))
        provenance = None
        if supplement is not None:
            vcs_path = home / '.cargo_vcs_info.json'
            if vcs_path.is_symlink():
                raise ValueError('VCS provenance must not be a symlink')
            vcs = json.loads(read_text(vcs_path))
            commit = supplement['commit']
            kind = supplement.get('provenance_kind', 'upstream-vcs')
            if kind == 'upstream-vcs':
                if not re.fullmatch(r'https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', supplement['repo']):
                    raise ValueError('Expected public pinned GitHub source')
            elif kind == 'distro-version-copyright':
                # This fixed reviewed index binds a distribution record to a crate
                # version, not to remotely verified upstream source bytes.
                distro_name = supplement['distro_package']
                distro_version = supplement['distro_version']
                if (distro_name != 'rust-' + name.replace('_', '-')
                        or distro_version != version + '-1'):
                    raise ValueError('Distribution record version mismatch')
                expected = (f'https://tracker.debian.org/media/packages/r/{distro_name}/copyright-{distro_version}',
                            f'https://changelogs.ubuntu.com/changelogs/pool/universe/r/{distro_name}/{distro_name}_{distro_version}/copyright')
                if supplement['source_url'] not in expected:
                    raise ValueError('Expected version-bound distribution copyright URL')
            else:
                raise ValueError('Unknown supplemental provenance kind')
            if (not re.fullmatch(r'[0-9a-f]{40}', commit)
                    or vcs.get('git', {}).get('sha1') != commit
                    or public_source(pkg.get('source')) not in (
                        'registry+https://github.com/rust-lang/crates.io-index',
                        'sparse+https://index.crates.io/')):
                raise ValueError('Supplement does not match the registry crate VCS commit')
            for text in supplement['texts']:
                relative = Path(text['path'])
                if relative.is_absolute() or '..' in relative.parts:
                    raise ValueError('Supplement path escapes pinned root')
                path = supplemental_root
                for part in relative.parts:
                    path = path / part
                    if path.is_symlink():
                        raise ValueError('Supplement path contains a symlink')
                data = read_text(path)
                if hashlib.sha256(data).hexdigest() != text['sha256']:
                    raise ValueError('Pinned upstream text hash mismatch')
                if kind == 'upstream-vcs':
                    if not text['url'].startswith(supplement['repo'].rstrip('/') + '/blob/' + commit + '/'):
                        raise ValueError('Supplement URL does not match its pinned source')
                elif text['url'] != supplement['source_url']:
                    raise ValueError('Distribution text URL mismatch')
                dest = f'dependency-notices/{name}-{version}/upstream/{relative.name}'
                if dest in files and files[dest] != data:
                    raise ValueError('Conflicting supplemental text')
                files[dest] = data
                if dest not in texts:
                    texts.append(dest)
            provenance = {'kind': kind, 'crate_commit': commit,
                          'texts': supplement['texts']}
            if kind == 'upstream-vcs':
                provenance['repo'] = supplement['repo']
                provenance['commit'] = commit
            else:
                provenance['distro_package'] = supplement['distro_package']
                provenance['distro_version'] = supplement['distro_version']
                provenance['source_url'] = supplement['source_url']
        rows.append({'name': name, 'version': version, 'source': public_source(pkg.get('source')),
                     'license': pkg.get('license'), 'texts': texts, 'upstream_provenance': provenance,
                     'status': ('distro-version-text-review-required' if provenance and provenance['kind'] == 'distro-version-copyright'
                                else 'texts-collected-review-required' if texts else 'missing-text-review-required')})
    files['dependency-notices/index.json'] = (json.dumps({
        'scope': 'normal/build metadata reachability, not final linked inclusion or legal clearance',
        'packages': rows}, indent=2) + '\n').encode()
    return files
