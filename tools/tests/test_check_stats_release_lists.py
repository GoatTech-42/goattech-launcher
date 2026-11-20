import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('check_stats', Path(__file__).parents[1] / 'check_stats_release_lists.py')
mod = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mod)


def rust(items):
    body = ', '.join(f'"{i}"' for i in items)
    return f'/// doc\npub const PUBLISHED_RELEASES: &[&str] = &[{body}];\n'


def js(items):
    body = ', '.join(f'"{i}"' for i in items)
    return f'const PUBLISHED_RELEASES = [{body}];\n'


class Lists(unittest.TestCase):
    def test_empty_lists_match(self):
        self.assertEqual(mod.check(rust([]), js([])), [])

    def test_same_releases_match_in_any_order(self):
        self.assertEqual(sorted(mod.check(rust(['v1.0.0', 'v1.1.0']), js(['v1.1.0', 'v1.0.0']))), ['v1.0.0', 'v1.1.0'])

    def test_client_only_and_worker_only_entries_fail_and_are_named(self):
        with self.assertRaisesRegex(ValueError, r"client only \['v1.0.0'\], worker only \[\]"):
            mod.check(rust(['v1.0.0']), js([]))
        with self.assertRaisesRegex(ValueError, r"client only \[\], worker only \['v1.0.0'\]"):
            mod.check(rust([]), js(['v1.0.0']))

    def test_missing_declaration_duplicates_and_non_literals_fail(self):
        with self.assertRaisesRegex(ValueError, 'not found'):
            mod.client_list('pub const OTHER: &[&str] = &[];')
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            mod.worker_list(js(['v1', 'v1']))
        with self.assertRaisesRegex(ValueError, 'string literals'):
            mod.worker_list('const PUBLISHED_RELEASES = [RELEASE_TAG];')
        with self.assertRaisesRegex(ValueError, 'string literals'):
            mod.client_list('pub const PUBLISHED_RELEASES: &[&str] = &[CURRENT];')

    def test_commented_declarations_are_not_active(self):
        decoy_js = '// const PUBLISHED_RELEASES = [];\n/* const PUBLISHED_RELEASES = []; */\n' + js(['v1.0.0'])
        with self.assertRaisesRegex(ValueError, r"client only \[\], worker only \['v1.0.0'\]"):
            mod.check(rust([]), decoy_js)
        decoy_rs = '// pub const PUBLISHED_RELEASES: &[&str] = &[];\n/* pub const PUBLISHED_RELEASES: &[&str] = &[]; */\n' + rust(['v1.0.0'])
        with self.assertRaisesRegex(ValueError, r"client only \['v1.0.0'\], worker only \[\]"):
            mod.check(decoy_rs, js([]))
        self.assertEqual(mod.check(decoy_rs, '// note\n' + js(['v1.0.0'])), ['v1.0.0'])

    def test_two_active_declarations_fail(self):
        with self.assertRaisesRegex(ValueError, 'more than one'):
            mod.worker_list(js([]) + js(['v1.0.0']))
        with self.assertRaisesRegex(ValueError, 'more than one'):
            mod.client_list(rust([]) + rust(['v1.0.0']))

    def test_comment_markers_inside_strings_are_kept(self):
        self.assertEqual(mod.worker_list(js(['https://x/y', 'v2'])), ['https://x/y', 'v2'])
        self.assertEqual(mod.client_list(rust(['a//b', 'v2'])), ['a//b', 'v2'])
        self.assertEqual(mod.worker_list('const PUBLISHED_RELEASES = ["v1" /* why */, "v2"]; // end\n'), ['v1', 'v2'])

    def test_the_real_files_match(self):
        # The permanent invariant: whatever is approved, both sides name the same releases.
        # (That both are empty before the first approved release is asserted by the Rust test
        # no_release_is_publishable_before_the_first_release, which changes with that release.)
        mod.check(mod.CLIENT.read_text(encoding='utf-8'), mod.WORKER.read_text(encoding='utf-8'))


if __name__ == '__main__':
    unittest.main()
