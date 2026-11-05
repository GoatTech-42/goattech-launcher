import re
import unittest
from pathlib import Path

AUDIT_PAGE = Path(__file__).resolve().parents[1] / 'docs' / 'engine' / 'upstream-parity-audit.md'


class ParityTests(unittest.TestCase):
    def test_readme_percentage_matches_table(self):
        text = AUDIT_PAGE.read_text()
        heading = re.search(r'^### Java parity: (\d+)% \(upstream self-audit, (\d{4}-\d{2}-\d{2})\)$', text, re.MULTILINE)
        self.assertIsNotNone(heading, 'README parity heading must say it is the dated upstream self-audit')
        table = text[heading.end():].split('| Subsystem | Score | Main gaps |', 1)[1].split('\n\n', 1)[0]
        scores = [float(score) for score in re.findall(r'^\| [^|]+ \| (0|0\.25|0\.5|0\.75|1) \| [^|]+ \|$', table, re.MULTILINE)]
        self.assertEqual(len(scores), table.count('\n|') - 1, 'Every row needs a score of 0, 0.25, 0.5, 0.75 or 1')
        self.assertEqual(int(heading.group(1)), round(sum(scores) * 100 / len(scores)))
        badge = re.search(r'badge/upstream%20tested%20parity%20[a-z]{3}%20\d{1,2}%20\d{4}-(\d+)%25-', text)
        self.assertIsNotNone(badge, 'README parity badge must say it is the dated upstream audit')
        self.assertEqual(badge.group(1), heading.group(1), 'The parity badge must match the table')


if __name__ == '__main__':
    unittest.main()
