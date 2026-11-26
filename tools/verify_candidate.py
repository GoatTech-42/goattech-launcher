"""Independently check candidate bytes and metadata inventory, not legal clearance."""
import argparse
import hashlib
import json
from pathlib import PurePosixPath, Path
import zipfile


def verify(archive, source, metadata):
    with zipfile.ZipFile(archive) as z:
        names = z.namelist()
        if len(names) != len(set(names)) or any(PurePosixPath(n).is_absolute() or '..' in PurePosixPath(n).parts or '\\' in n for n in names):
            raise ValueError('Unsafe or duplicate archive path')
        manifest = json.loads(z.read('manifest.json'))
        if manifest['source_commit'] != source or manifest['platform'] != 'windows-x64':
            raise ValueError('Unexpected source or platform')
        if set(names) != set(manifest['sha256']) | {'manifest.json'}:
            raise ValueError('Archive inventory does not match manifest')
        for name, digest in manifest['sha256'].items():
            if hashlib.sha256(z.read(name)).hexdigest() != digest:
                raise ValueError('Payload hash mismatch: ' + name)
        index = json.loads(z.read('dependency-notices/index.json'))
        rows = index['packages']
        actual = {(r['name'], r['version']): r for r in rows}
        # Walk the metadata graph independently, rather than reuse the collector.
        nodes = {n['id']: n for n in metadata['resolve']['nodes']}
        reached = {metadata['resolve']['root']}
        while True:
            expanded = reached | {d['pkg'] for key in reached for d in nodes[key]['deps']
                                  if any(k['kind'] != 'dev' for k in d['dep_kinds'])}
            if expanded == reached:
                break
            reached = expanded
        expected = {(p['name'], p['version']): p for p in metadata['packages'] if p['id'] in reached}
        if len(rows) != len(actual) or set(actual) != set(expected):
            raise ValueError('Incomplete or duplicate dependency inventory')
        unresolved = []
        for identity, row in actual.items():
            if row['license'] != expected[identity].get('license'):
                raise ValueError('Dependency license expression mismatch')
            for name in row['texts']:
                if name not in manifest['sha256'] or not name.startswith('dependency-notices/') or not z.read(name).strip():
                    raise ValueError('Missing or empty dependency notice')
            if not row['texts'] or row['status'] != 'texts-collected-review-required':
                unresolved.append('@'.join(identity))
        return {'source': source, 'archive_sha256': hashlib.sha256(Path(archive).read_bytes()).hexdigest(),
                'payload_count': len(manifest['sha256']), 'dependency_count': len(expected),
                'unresolved_notices': sorted(unresolved), 'legal_clearance': False,
                'scope': 'payload integrity and normal/build metadata inventory, not linked-code or legal audit'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--metadata', required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.archive, args.source, json.loads(Path(args.metadata).read_text(encoding='utf-8'))), indent=2))
