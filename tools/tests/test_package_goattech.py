"""Portable packages must contain only explicit public inputs, never a profile."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location('package_goattech', Path(__file__).parents[1] / 'package_goattech.py')


class Packages(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.binary = self.root / 'input.exe'
        self.binary.write_bytes(b'MZpublic-binary')
        (self.root / 'account.json').write_text('PRIVATE')
        self.out = self.root / 'out'
        self.out.mkdir()
        self.spec = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(self.spec)

    def package(self, platform='windows-x64', version='0.1.0'):
        return self.spec.package(self.binary, self.out, platform, version, 'a' * 40)

    def test_windows_contains_explicit_inputs_only_and_verifiable_manifest(self):
        archive = self.package()
        with zipfile.ZipFile(archive) as z:
            self.assertEqual(set(z.namelist()), {'horizon-launcher.exe', 'README.txt', 'LICENSE', 'goattech.svg', 'manifest.json', 'THIRD_PARTY_NOTICES.md', 'notices/OFL-Geist.txt', 'notices/OFL-BricolageGrotesque.txt', 'notices/OFL-PixelifySans.txt', 'notices/LICENSE-lucide.txt', 'notices/LICENSE-pixelarticons.txt', 'notices/naga-LICENSE.MIT', 'notices/naga-LICENSE.APACHE', 'notices/wgpu-hal-LICENSE.MIT', 'notices/wgpu-hal-LICENSE.APACHE'})
            manifest = json.loads(z.read('manifest.json'))
            self.assertEqual(manifest['platform'], 'windows-x64')
            self.assertEqual(manifest['version'], '0.1.0')
            self.assertEqual(manifest['source_commit'], 'a' * 40)
            for name, digest in manifest['sha256'].items():
                self.assertEqual(hashlib.sha256(z.read(name)).hexdigest(), digest)
            self.assertNotIn(b'PRIVATE', b''.join(z.read(n) for n in z.namelist()))
        sums = (self.out / 'SHA256SUMS').read_text()
        self.assertEqual(sums, hashlib.sha256(archive.read_bytes()).hexdigest() + '  ' + archive.name + '\n')

    def test_notices_preserve_original_license_texts(self):
        with zipfile.ZipFile(self.package()) as z:
            self.assertEqual(z.read('notices/OFL-Geist.txt'), (Path(__file__).parents[2] / 'crates/horizon-theme/fonts/OFL-Geist.txt').read_bytes())
            self.assertEqual(z.read('THIRD_PARTY_NOTICES.md'), (Path(__file__).parents[2] / 'THIRD_PARTY_NOTICES.md').read_bytes())
            self.assertIn('notices/LICENSE-lucide.txt', json.loads(z.read('manifest.json'))['sha256'])

    def test_linux_binary_has_executable_mode(self):
        self.binary.write_bytes(b'\x7fELFpublic-binary')
        with tarfile.open(self.package('linux-x64')) as t:
            self.assertEqual(t.getmember('horizon-launcher').mode, 0o755)
            self.assertEqual(t.extractfile('horizon-launcher').read(), b'\x7fELFpublic-binary')

    def test_existing_outputs_are_never_overwritten(self):
        archive = self.package()
        before = archive.read_bytes()
        with self.assertRaises(FileExistsError):
            self.package()
        self.assertEqual(archive.read_bytes(), before)

    def test_bad_identity_and_unsupported_platform_refused(self):
        for platform, version, commit in [('macos', '0.1.0', 'a'*40), ('windows-x64','../x','a'*40), ('windows-x64','0.1.0','main')]:
            with self.assertRaises(ValueError):
                self.spec.package(self.binary, self.out, platform, version, commit)
        self.assertEqual(list(self.out.iterdir()), [])

    def test_symlink_binary_refused(self):
        link = self.root / 'link'
        link.symlink_to(self.binary)
        with self.assertRaises(ValueError):
            self.spec.package(link, self.out, 'windows-x64', '0.1.0', 'a'*40)
        self.assertEqual(list(self.out.iterdir()), [])

    def test_invalid_commit_does_not_leave_partial_archive(self):
        with self.assertRaises(ValueError):
            self.spec.package(self.binary, self.out, "windows-x64", "0.1.0", "main")
        self.assertEqual(list(self.out.iterdir()), [])

    def test_wrong_binary_platform_refused(self):
        with self.assertRaises(ValueError):
            self.package('linux-x64')
        self.assertEqual(list(self.out.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
