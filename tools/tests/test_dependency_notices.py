"""Notice collection must preserve text and unresolved obligations without leaking paths."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


class Notices(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        spec = importlib.util.spec_from_file_location('dependency_notices', Path(__file__).parents[1] / 'dependency_notices.py')
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.pkg = self.root / 'crate'
        self.pkg.mkdir()
        (self.pkg / 'Cargo.toml').write_text('')
        self.metadata = {'packages': [{'id':'crate-id','name':'example','version':'1.2.3','license':'MIT AND ISC','license_file':None,'source':'registry+https://example.invalid/index','manifest_path':str(self.pkg / 'Cargo.toml')}], 'resolve':{'root':'crate-id','nodes':[{'id':'crate-id','deps':[]}]}}

    def collect(self):
        return self.module.collect(self.metadata, [self.root])

    def test_preserves_text_expression_and_missing_status_without_private_paths(self):
        (self.pkg / 'LICENSE-MIT').write_text('Original copyright and license\n')
        files = self.collect()
        self.assertEqual(files['dependency-notices/example-1.2.3/LICENSE-MIT'], b'Original copyright and license\n')
        index = json.loads(files['dependency-notices/index.json'])
        self.assertEqual(index['packages'][0]['license'], 'MIT AND ISC')
        self.assertEqual(index['packages'][0]['status'], 'texts-collected-review-required')
        self.assertNotIn(str(self.root), files['dependency-notices/index.json'].decode())
        (self.pkg / 'LICENSE-MIT').unlink()
        self.assertEqual(json.loads(self.collect()['dependency-notices/index.json'])['packages'][0]['status'], 'missing-text-review-required')

    def test_symlink_or_escape_license_is_refused(self):
        outside = self.root / 'private.txt'
        outside.write_text('PRIVATE')
        (self.pkg / 'LICENSE').symlink_to(outside)
        with self.assertRaises(ValueError):self.collect()
        (self.pkg / 'LICENSE').unlink()
        self.metadata['packages'][0]['license_file'] = '../private.txt'
        with self.assertRaises(ValueError):self.collect()

    def test_private_and_credential_sources_never_enter_public_index(self):
        for source in ['git+file:///home/private/repo#abc', 'git+https://token:secret@example.com/repo#abc', 'git+https://example.com/repo?token=secret#abc', 'registry+http://private.local/index']:
            self.metadata['packages'][0]['source'] = source
            row = json.loads(self.collect()['dependency-notices/index.json'])['packages'][0]
            self.assertEqual(row['source'], 'redacted-unverified-source')
            self.assertNotIn('secret', json.dumps(row))
            self.assertNotIn('/home/private', json.dumps(row))
        self.metadata['packages'][0]['source'] = 'registry+https://github.com/rust-lang/crates.io-index'
        self.assertEqual(json.loads(self.collect()['dependency-notices/index.json'])['packages'][0]['source'], 'registry+https://github.com/rust-lang/crates.io-index')

    def test_dev_only_dependency_is_not_collected(self):
        self.metadata['resolve']['nodes'][0]['deps'] = [{'pkg':'absent-dev-id','dep_kinds':[{'kind':'dev'}]}]
        self.assertEqual(len(json.loads(self.collect()['dependency-notices/index.json'])['packages']),1)

    def test_metadata_outside_allowed_roots_is_refused(self):
        (self.root / 'elsewhere').mkdir()
        with self.assertRaises(ValueError):self.module.collect(self.metadata, [self.root / 'elsewhere'])


if __name__ == '__main__': unittest.main()
