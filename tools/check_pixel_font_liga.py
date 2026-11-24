#!/usr/bin/env python3
"""Check bundled Pixelify GSUB workaround. Requires fontTools (development only)."""
from pathlib import Path
import hashlib
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[1]
for weight in ("Medium", "SemiBold", "Bold"):
    path = ROOT / "crates/horizon-theme/fonts" / f"PixelifySans-{weight}.ttf"
    font = TTFont(path)
    features = font["GSUB"].table.FeatureList.FeatureRecord
    liga = [r.Feature for r in features if r.FeatureTag == "liga"]
    assert liga and all(not f.LookupListIndex and f.LookupCount == 0 for f in liga), path
    for name in ("f", "i", "l"):
        assert ord(name) in font.getBestCmap(), (path, name)
    if weight != "Medium":
        site = ROOT / "site/assets/fonts" / path.name
        assert site.read_bytes() == path.read_bytes(), site
    print(f"PASS {path.name}: liga disabled; basic letters mapped; site copy consistent")
for line in (ROOT / "site/SHA256SUMS").read_text().splitlines():
    digest, name = line.split("  ", 1)
    if "PixelifySans" in name:
        assert hashlib.sha256((ROOT / "site" / name).read_bytes()).hexdigest() == digest, name
print("PASS site Pixelify font/license hashes")
