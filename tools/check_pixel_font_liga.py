#!/usr/bin/env python3
"""Check bundled Pixelify GSUB workaround. Requires fontTools (development only)."""
from pathlib import Path
import hashlib
import io
import subprocess
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[1]
for weight in ("Medium", "SemiBold", "Bold"):
    path = ROOT / "crates/horizon-theme/fonts" / f"PixelifySans-{weight}.ttf"
    font = TTFont(path)
    features = font["GSUB"].table.FeatureList.FeatureRecord
    liga = [r.Feature for r in features if r.FeatureTag == "liga"]
    assert liga and all(not f.LookupListIndex and f.LookupCount == 0 for f in liga), path
    h = font["glyf"]["H"]
    assert h.numberOfContours == 1 and len(h.coordinates) == 12, path
    assert len(h.program.getBytecode()) == 0, path
    points = list(h.coordinates)
    assert all(a[0] == b[0] or a[1] == b[1] for a, b in zip(points, points[1:] + points[:1])), path
    assert len({y for _, y in points}) == 4, path
    baseline_path = path.relative_to(ROOT).as_posix()
    original = TTFont(io.BytesIO(subprocess.check_output([
        "git", "show", "3ce5c220cf79fabd5e001f164f31291943c263ff:" + baseline_path
    ], cwd=ROOT)))
    assert original.getGlyphOrder() == font.getGlyphOrder(), path
    changed = [g for g in original.getGlyphOrder()
               if original["glyf"][g].compile(original["glyf"]) != font["glyf"][g].compile(font["glyf"])]
    assert changed == ["H"], (path, changed)
    assert original["hmtx"].metrics == font["hmtx"].metrics, path
    for tag in original.keys():
        if tag not in {"GlyphOrder", "GSUB", "head", "loca", "glyf"}:
            assert original.getTableData(tag) == font.getTableData(tag), (path, tag)
    for record in original["GSUB"].table.FeatureList.FeatureRecord:
        if record.FeatureTag == "liga":
            record.Feature.LookupListIndex = []
            record.Feature.LookupCount = 0
    assert original.getTableData("GSUB") == font.getTableData("GSUB"), path
    for name in ("f", "i", "l"):
        assert ord(name) in font.getBestCmap(), (path, name)
    if weight != "Medium":
        site = ROOT / "site/assets/fonts" / path.name
        assert site.read_bytes() == path.read_bytes(), site
    print(f"PASS {path.name}: liga disabled; only H glyph changed; straight unhinted H; metrics preserved; site copy consistent")
for line in (ROOT / "site/SHA256SUMS").read_text().splitlines():
    digest, name = line.split("  ", 1)
    if "PixelifySans" in name:
        assert hashlib.sha256((ROOT / "site" / name).read_bytes()).hexdigest() == digest, name
print("PASS site Pixelify font/license hashes")
