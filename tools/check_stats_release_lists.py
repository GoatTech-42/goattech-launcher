#!/usr/bin/env python3
"""Fail unless the basic-stats release allowlists of the client and the collector match.

The client (crates/horizon-telemetry/src/basic.rs, PUBLISHED_RELEASES) and the Cloudflare worker
(infra/telemetry-worker/src/index.js, PUBLISHED_RELEASES) must name exactly the same releases,
or reports are dropped or accepted on one side only. Both lists stay empty until the first
approved release. Read-only: this only parses the two source files.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLIENT = ROOT / 'crates/horizon-telemetry/src/basic.rs'
WORKER = ROOT / 'infra/telemetry-worker/src/index.js'
STRING = re.compile(r'"([^"\\]*)"')


def strip_comments(text, quotes):
    """Drop // and /* */ comments, leaving string literals (in `quotes`) untouched."""
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c in quotes:
            j = i + 1
            while j < n and text[j] != c:
                j += 2 if text[j] == '\\' else 1
            out.append(text[i:j + 1])
            i = j + 1
        elif text.startswith('//', i):
            j = text.find('\n', i)
            i = n if j < 0 else j
        elif text.startswith('/*', i):
            j = text.find('*/', i + 2)
            if j < 0:
                raise ValueError('unterminated block comment')
            out.append(' ')
            i = j + 2
        else:
            out.append(c)
            i += 1
    return ''.join(out)


def _list(text, pattern, quotes, what):
    matches = list(re.finditer(pattern, strip_comments(text, quotes), re.S))
    if not matches:
        raise ValueError(f'{what}: PUBLISHED_RELEASES declaration not found')
    if len(matches) > 1:
        raise ValueError(f'{what}: more than one active PUBLISHED_RELEASES declaration')
    body = matches[0].group(1)
    items = STRING.findall(body)
    # Only plain string literals are allowed: anything else would be silently skipped.
    leftover = STRING.sub('', body).replace(',', '').strip()
    if leftover:
        raise ValueError(f'{what}: PUBLISHED_RELEASES holds something other than string literals')
    if len(set(items)) != len(items):
        raise ValueError(f'{what}: PUBLISHED_RELEASES has a duplicate entry')
    return items


def client_list(text):
    return _list(text, r'pub const PUBLISHED_RELEASES:\s*&\[&str\]\s*=\s*&\[(.*?)\];', '"', 'client')


def worker_list(text):
    return _list(text, r'const PUBLISHED_RELEASES\s*=\s*\[(.*?)\];', '"\'`', 'worker')


def check(client_text, worker_text):
    client, worker = client_list(client_text), worker_list(worker_text)
    if sorted(client) != sorted(worker):
        only_client, only_worker = sorted(set(client) - set(worker)), sorted(set(worker) - set(client))
        raise ValueError(f'release lists differ: client only {only_client}, worker only {only_worker}')
    return client


def main():
    releases = check(CLIENT.read_text(encoding='utf-8'), WORKER.read_text(encoding='utf-8'))
    print(f'basic-stats release lists match ({len(releases)} release(s): {", ".join(releases) or "none"})')


if __name__ == '__main__':
    try:
        main()
    except ValueError as error:
        sys.exit(f'error: {error}')
