#!/usr/bin/env python3
"""Offline p41 capture check. Pillow required. Run on decoded original PNGs only.

Scan the upper-middle client region, away from native borders and bottom label.
Strict RGB equality is intentional. Bright components separated across the frame
reject blank backgrounds and a single bright artifact, not every possible forgery.
This is an isolated-sky fixture check, not a whole-launcher image classifier.
"""
import argparse
import json
from pathlib import Path
from PIL import Image


def inspect(path):
    with Image.open(path) as image:
        image.load()
        width, height = image.size
        if width < 200 or height < 200 or width > 3840 or height > 3840:
            raise ValueError('image geometry outside fixture bounds')
        rgb = image.convert('RGB')
        left, top = max(24, int(width * .03)), max(24, int(height * .05))
        right, bottom = int(width * .97), int(height * .45)
        bright = set()
        tinted = []
        for y in range(top, bottom):
            for x in range(left, right):
                r, g, b = rgb.getpixel((x, y))
                if r != g or g != b:
                    if len(tinted) < 8:
                        tinted.append([x, y, r, g, b])
                if r == g == b and r >= 100:
                    bright.add((x, y))
        if tinted:
            raise ValueError(f'non-neutral sampled pixels: {tinted}')
        components = []
        while bright:
            todo = [bright.pop()]
            component = []
            while todo:
                x, y = todo.pop()
                component.append((x, y))
                for neighbor in ((x-1,y),(x+1,y),(x,y-1),(x,y+1)):
                    if neighbor in bright:
                        bright.remove(neighbor)
                        todo.append(neighbor)
            components.append(component)
        # Real stars are small isolated quads. Background is dark, without text here.
        stars = [c for c in components if 1 <= len(c) <= 256]
        if len(stars) < 4:
            raise ValueError(f'not enough visible star components: {len(stars)}')
        centers = [(sum(x for x,y in c)/len(c), sum(y for x,y in c)/len(c)) for c in stars]
        if max(x for x,y in centers)-min(x for x,y in centers) < (right-left)/4:
            raise ValueError('bright components not spread across sky')
        return {'file': str(path), 'size': [width,height], 'region': [left,top,right,bottom],
                'neutral': True, 'star_components': len(stars)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('png', nargs='+', type=Path)
    args = parser.parse_args()
    failed = False
    for path in args.png:
        try:
            print(json.dumps(inspect(path)))
        except (ValueError, OSError) as error:
            failed = True
            print(json.dumps({'file': str(path), 'pass': False, 'error': str(error)}))
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
