"""Check actual glyph geometry and layout metrics, not a screenshot snapshot.

Run with the stock font path to reproduce the H notch regression.
"""
import sys
import unittest
from pathlib import Path

from fontTools.pens.pointInsidePen import PointInsidePen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'crates/horizon-theme/fonts/PixelifySans-Bold.ttf'
FONT = Path(sys.argv.pop(1)) if len(sys.argv) > 1 else ROOT / 'launcher/assets/fonts/GoatTechOpening-Bold.ttf'


class OpeningFont(unittest.TestCase):
    def setUp(self):
        self.font = TTFont(FONT)
        self.source = TTFont(SOURCE)

    def tearDown(self):
        self.font.close()
        self.source.close()

    def test_h_has_open_top_counter_above_a_straight_crossbar(self):
        glyphs = self.font.getGlyphSet()
        h = glyphs[self.font.getBestCmap()[ord('H')]]
        for point, want in [((301, 420), False), ((301, 310), True),
                            ((120, 500), True), ((470, 500), True),
                            ((301, 150), False)]:
            pen = PointInsidePen(glyphs, point)
            h.draw(pen)
            self.assertEqual(pen.getResult(), want, f'H fill at {point}')

    def test_other_wordmark_glyphs_and_all_advance_widths_are_unchanged(self):
        for char in 'GOATECH':
            a = self.font.getBestCmap()[ord(char)]
            b = self.source.getBestCmap()[ord(char)]
            self.assertEqual(self.font['hmtx'][a], self.source['hmtx'][b], char)
            if char != 'H':
                self.assertEqual(self.font['glyf'][a].getCoordinates(self.font['glyf']),
                                 self.source['glyf'][b].getCoordinates(self.source['glyf']), char)
        for table, attrs in [('head', ['unitsPerEm']), ('hhea', ['ascent', 'descent', 'lineGap']),
                             ('OS/2', ['usWeightClass', 'sTypoAscender', 'sTypoDescender'])]:
            for attr in attrs:
                self.assertEqual(getattr(self.font[table], attr), getattr(self.source[table], attr))


if __name__ == '__main__':
    unittest.main()
