import re
import unittest
from datetime import date
from pathlib import Path

from tools import parity_gaps as pg

AUDIT_PAGE = Path(__file__).resolve().parents[1] / "docs" / "engine" / "upstream-parity-audit.md"


def lists(*opens):
    return [{"number": 3512 + i, "area": f"Area {i}", "open": n, "closed": 1} for i, n in enumerate(opens)]


class CountTests(unittest.TestCase):
    def test_rows(self):
        body = "Intro\n\n- [ ] `A.b` (missing): x\n- [x] `C.d` (gap): y\n- [X] `E.f`\n* [ ] `G.h`\n  - [ ] nested\n"
        self.assertEqual(pg.rows(body), (3, 2))
        self.assertEqual(pg.rows(None), (0, 0))

    def test_area_from_the_title(self):
        self.assertEqual(pg.area("CRUCIAL: Player-visible port gaps: Blocks, redstone and fluids (86)"),
                         "Blocks, redstone and fluids")


class ReadmeTests(unittest.TestCase):
    def test_block_round_trip(self):
        block = pg.block(lists(5, 20, 0))
        self.assertIn("**Player-visible gaps: 25 open** of the 28 ", block)
        self.assertLess(block.index("Area 1 [20]"), block.index("Area 0 [5]"))  # the biggest first
        self.assertEqual(pg.written(block), (25, [20, 5, 0]))

    def test_rewrite_replaces_the_block_and_the_badge(self):
        text = ("<p>\n  <img src=\"https://img.shields.io/badge/upstream%20tested%20parity%20oct%2013%202026-100%25-F0795A\" alt=\"x\">\n"
                '  <a href="#project-status"><img src="https://img.shields.io/badge/upstream%20gaps%20oct%2030%202026%20snapshot-9%20open-F0795A" alt="Upstream gaps, oct 30 2026 snapshot"></a>\n</p>\n\n'
                "## Project status\n\nThe table.\n")
        once = pg.rewrite(text, pg.block(lists(7)), 7)
        self.assertIn(pg.badge_label(7), once)
        self.assertNotIn("oct%2030%202026", once, "a refresh re-dates the badge with the count")
        self.assertIn("## Project status\n\n**Player-visible gaps: 7 open**", once)
        twice = pg.rewrite(once, pg.block(lists(3)), 3)
        self.assertEqual(twice.count("**Player-visible gaps:"), 1)
        self.assertIn(pg.badge_label(3), twice)
        self.assertEqual(pg.written(twice)[0], 3)
        badge_date = re.search(r"upstream%20gaps%20([a-z]{3}%20\d{1,2}%20\d{4})%20snapshot", twice).group(1)
        self.assertIn(f"Counts from upstream's tracker on {date.today().isoformat()}", twice)
        self.assertEqual(badge_date, f"{date.today():%b}".lower() + f"%20{date.today().day}%20{date.today():%Y}",
                         "badge and block must carry the same snapshot date")

    def test_the_readme_agrees_with_itself(self):
        text = AUDIT_PAGE.read_text(encoding="utf-8")
        total, counts = pg.written(text)
        self.assertEqual(len(counts), len(pg.ISSUES))
        self.assertEqual(total, sum(counts), "the README's gap total is the sum of its areas")
        badge = pg.BADGE.search(text)
        self.assertIsNotNone(badge, "the README's gap badge must carry its snapshot date")
        self.assertEqual(int(badge.group(2)), total, "the gap badge must match the block")
        status = text.split("## Project status", 1)[1].split("\n", 1)[1]
        intro, _, rest = status.lstrip().partition("\n\n")
        self.assertIn("archived upstream status", intro)
        self.assertIn("not this fork's current status", intro)
        self.assertIn("has not re-audited", intro)
        self.assertIsNone(re.search(r"\d{4}", intro),
                          "the archive note pins no year; the figures carry their own dates")
        self.assertTrue(rest.lstrip().startswith("**Player-visible gaps:"),
                        "the gaps follow the archived-status note under Project status")
        self.assertNotIn("<!--", status.split("###", 1)[0], "the website prints HTML comments of this intro")
        self.assertTrue(re.search(r"^### Java parity: \d+% \(upstream self-audit, \d{4}-\d{2}-\d{2}\)$", status, re.M),
                        "the site still reads this heading, with its audit date")


if __name__ == "__main__":
    unittest.main()
