"""The package verifier must reject missing, modified and unlisted payloads."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile


class VerifyCandidate(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        spec = importlib.util.spec_from_file_location('verify_candidate', Path(__file__).parents[1] / 'verify_candidate.py')
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.files = {'horizon-launcher.exe': b'MZfixture', 'dependency-notices/one/LICENSE': b'Original MIT text',
                      'notices/OFL-Geist.txt': b'OFL fixture', 'LICENSE': b'MIT fixture'}
        self.index = {'packages': [{'name': 'one', 'version': '1.0', 'license': 'MIT',
                                  'texts': ['dependency-notices/one/LICENSE'], 'status': 'texts-collected-review-required'}]}
        self.metadata = {'packages': [{'id': 'root', 'name': 'one', 'version': '1.0', 'license': 'MIT'}],
                         'resolve': {'root': 'root', 'nodes': [{'id': 'root', 'deps': []}]}}

    def make(self, change=None):
        files = dict(self.files)
        files['dependency-notices/index.json'] = json.dumps(self.index).encode()
        manifest = {'source_commit': 'a'*40, 'platform': 'windows-x64',
                    'sha256': {n: hashlib.sha256(b).hexdigest() for n,b in files.items()}}
        files['manifest.json'] = json.dumps(manifest).encode()
        if change:
            change(files)
        archive = self.root / 'candidate.zip'
        with zipfile.ZipFile(archive, 'w') as z:
            for n,b in files.items():
                z.writestr(n,b)
        return archive

    def test_valid_payload_and_inventory_are_bound_to_expected_source(self):
        report = self.module.verify(self.make(), 'a'*40, self.metadata)
        self.assertEqual(report['dependency_count'], 1)
        self.assertEqual(report['unresolved_notices'], [])
        self.assertFalse(report['legal_clearance'])

    def test_modified_binary_fails(self):
        with self.assertRaisesRegex(ValueError, 'hash'):
            self.module.verify(self.make(lambda f: f.update({'horizon-launcher.exe': b'MZmodified'})), 'a'*40, self.metadata)

    def test_unlisted_account_file_fails(self):
        with self.assertRaisesRegex(ValueError, 'inventory'):
            self.module.verify(self.make(lambda f: f.update({'account.json': b'private'})), 'a'*40, self.metadata)

    def test_wrong_source_fails(self):
        with self.assertRaisesRegex(ValueError, 'source'):
            self.module.verify(self.make(), 'b'*40, self.metadata)

    def test_missing_dependency_is_detected_independently_of_package_index(self):
        self.metadata['packages'].append({'id': 'second', 'name': 'two', 'version': '2.0', 'license': 'ISC'})
        self.metadata['resolve']['nodes'][0]['deps'] = [{'pkg': 'second', 'dep_kinds': [{'kind': None}]}]
        self.metadata['resolve']['nodes'].append({'id': 'second', 'deps': []})
        with self.assertRaisesRegex(ValueError, 'dependency inventory'):
            self.module.verify(self.make(), 'a'*40, self.metadata)

    def test_missing_license_text_remains_unresolved(self):
        self.index['packages'][0]['texts'] = []
        self.index['packages'][0]['status'] = 'missing-text-review-required'
        report = self.module.verify(self.make(), 'a'*40, self.metadata)
        self.assertEqual(report['unresolved_notices'], ['one@1.0'])

    def test_missing_referenced_notice_fails_even_when_manifest_is_consistent(self):
        del self.files['dependency-notices/one/LICENSE']
        with self.assertRaisesRegex(ValueError, 'notice'):
            self.module.verify(self.make(), 'a'*40, self.metadata)

    def test_changed_license_expression_fails(self):
        self.index['packages'][0]['license'] = 'Apache-2.0'
        with self.assertRaisesRegex(ValueError, 'license'):
            self.module.verify(self.make(), 'a'*40, self.metadata)

    def test_path_traversal_fails(self):
        self.files['../escape'] = b'bad'
        with self.assertRaisesRegex(ValueError, 'path'):
            self.module.verify(self.make(), 'a'*40, self.metadata)


if __name__ == '__main__':
    unittest.main()
