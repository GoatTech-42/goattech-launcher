#!/usr/bin/env python3
"""Keep Pixelify's wordmark geometry except the H's M-like central notch.

fontTools is needed only to regenerate this OFL derivative, not to build the app.
The separate family and intro-only registration leave every other heading unchanged.
"""
from pathlib import Path
from fontTools import subset
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
font = TTFont(ROOT / 'crates/horizon-theme/fonts/PixelifySans-Bold.ttf', recalcTimestamp=False)
glyph = font.getBestCmap()[ord('H')]
pen = TTGlyphPen(None)
pen.moveTo((61, -11))
for point in [(61, 638), (188, 638), (188, 381), (414, 381), (414, 638),
              (542, 638), (542, -11), (414, -11), (414, 246), (188, 246), (188, -11)]:
    pen.lineTo(point)
pen.closePath()
font['glyf'][glyph] = pen.glyph()
family = 'GoatTech Opening'
for record in font['name'].names:
    if record.nameID in (1, 16): record.string = family.encode(record.getEncoding())
    elif record.nameID in (3, 4, 6):
        value = 'GoatTechOpening-Bold' if record.nameID in (3, 6) else family + ' Bold'
        record.string = value.encode(record.getEncoding())
options = subset.Options()
options.name_IDs = ['*']
options.name_legacy = True
options.name_languages = ['*']
subsetter = subset.Subsetter(options=options)
subsetter.populate(text='GOATTECH')
subsetter.subset(font)
font.save(ROOT / 'launcher/assets/fonts/GoatTechOpening-Bold.ttf')
