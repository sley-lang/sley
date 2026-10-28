#!/usr/bin/env python3
"""Focused offline mutation tests; never contact the website."""
import contextlib
from email.message import Message
from http.client import HTTPResponse
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_public_entrypoints as c

VERSION = '2.1.1'
README = '''<img alt="Version 2.1.1" src="https://img.shields.io/badge/version-2.1.1-38c8e8?style=flat-square">
[Release notes](docs/release/SLEY-2.1.1.md)
## Quickstart
Download sley-2.1.1-linux-x86_64.tar.gz from
[Releases](https://github.com/sley-lang/sley/releases/tag/v2.1.1).
## History
[2.0.0 release notes](docs/release/SLEY-2.0.0.md)
[self-hosting record](docs/release/SH2_SELF_HOSTING.md)
'''
QUICKSTART = '''## 1. Install
Sley 2.1.1 supports Linux x86_64.
sley-2.1.1-linux-x86_64.tar.gz
[Releases](https://github.com/sley-lang/sley/releases/tag/v2.1.1)
[release notes](release/SLEY-2.1.1.md)
## 2. Check the binary
{"cli":"1","protocol_version":1}
'''
INDEX = '''# Documentation
Current release: [Sley 2.1.1](release/SLEY-2.1.1.md).
## History
[Sley 2.0.0 release notes](release/SLEY-2.0.0.md)
'''
LIVE = '''<p>Sley 2.1.1 is available. Version 2.1.1.</p>
<p><a href="https://github.com/sley-lang/sley/releases/tag/v2.1.1">Download Sley 2.1.1</a></p>
<p>sley-2.1.1-linux-x86_64.tar.gz</p>
<p><a href="https://github.com/sley-lang/sley/releases/tag/v2.1.1">release page</a></p>
<p><a href="https://github.com/sley-lang/sley/blob/v2.1.1/docs/QUICKSTART.md">Quickstart</a></p>
<p><a href="https://github.com/sley-lang/sley/blob/v2.1.1/docs/release/SLEY-2.1.1.md">release notes</a></p>
'''
HISTORY = '''<p>Self-rebuild bundle sley-2.1.0-selfhost-linux-x86_64.tar.gz from the
<a href="https://github.com/sley-lang/sley/releases/tag/v2.1.0">2.1.0 release</a>.</p>
<p><a href="https://github.com/sley-lang/sley/blob/v2.1.0/docs/release/SH2_SELF_HOSTING.md">self-hosting record</a></p>
<p><a href="https://github.com/sley-lang/sley/blob/v2.0.0/docs/release/SLEY-2.0.0.md">2.0.0 release notes</a></p>
<p>Sley 1.2.1 is the final human-readable release at GreyforgeLabs/sley-legacy.</p>
'''


def http_response(headers, body):
    """Exercise the stdlib decoder using HTTP wire bytes, without a network."""
    class Socket:
        def makefile(self, *args):
            return io.BytesIO(b'HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n'
                              + headers + b'\r\n' + body)

    response = HTTPResponse(Socket())
    response.begin()
    return response


class EntryPoints(unittest.TestCase):
    def local(self, name, text, expected='PASS'):
        report = c.check_local(name, text, VERSION)
        self.assertEqual(report['status'], expected, report)

    def live(self, text, expected='PASS'):
        report = c.check_live('/', text, VERSION)
        self.assertEqual(report['status'], expected, report)

    def test_valid_offline_and_historical_notes(self):
        for name, text in [('README.md', README), ('docs/QUICKSTART.md', QUICKSTART), ('docs/README.md', INDEX)]:
            self.local(name, text)
        self.local('README.md', README + '[historical selfhost release](https://github.com/sley-lang/sley/releases/tag/v2.1.0)\n')

    def test_stale_badge_or_archive_despite_current_notes(self):
        for before in ['Version 2.1.1', 'version-2.1.1', 'sley-2.1.1-linux']:
            self.local('README.md', README.replace(before, before.replace('2.1.1', '2.0.1')), 'FAIL')

    def test_stale_readme_download_tag_and_wrong_repository(self):
        self.local('README.md', README.replace('/tag/v2.1.1', '/tag/v2.0.1'), 'FAIL')
        self.local('README.md', README.replace('/sley/releases', '/wrong/releases'), 'FAIL')

    def test_stale_quickstart_install_archive_and_tag(self):
        for before in ['Sley 2.1.1 supports', 'sley-2.1.1-linux', '/tag/v2.1.1']:
            self.local('docs/QUICKSTART.md', QUICKSTART.replace(before, before.replace('2.1.1', '2.0.1')), 'FAIL')

    def test_historical_link_cannot_replace_primary(self):
        self.local('README.md', README.replace('docs/release/SLEY-2.1.1.md', 'docs/release/SLEY-2.0.0.md'), 'FAIL')
        self.local('docs/README.md', INDEX.replace('Current release: [Sley 2.1.1]', 'Current release: [Sley 2.0.1]'), 'FAIL')
        self.local('docs/README.md', INDEX.replace('release/SLEY-2.1.1.md', 'release/SLEY-2.0.1.md'), 'FAIL')
        self.local('README.md', README.replace('docs/release/SLEY-2.1.1.md', 'https://example.org/SLEY-2.1.1.md'), 'FAIL')
        self.local('docs/QUICKSTART.md', QUICKSTART.replace('release/SLEY-2.1.1.md', 'wrong/SLEY-2.1.1.md'), 'FAIL')

    def test_valid_live_and_explicit_history(self):
        self.live(LIVE)
        self.live(LIVE + HISTORY)

    def test_live_stale_claim_does_not_hide_behind_expected(self):
        self.live(LIVE.replace('Version 2.1.1', 'Version 2.0.1'), 'FAIL')
        self.live(LIVE + '<p>sley-2.0.1-linux-x86_64.tar.gz</p>', 'FAIL')
        self.live(LIVE + '<p>2.0.1 current release</p>', 'FAIL')
        self.live(LIVE + '<p>Current release: Sley 2.0.1</p>', 'FAIL')

    def test_major_one_claim_requires_explicit_legacy_context(self):
        self.live(LIVE + '<p>Sley 1.2.1 is the current version.</p>', 'FAIL')
        self.live(LIVE + '<p>Sley 1.2.1 is available.</p>', 'FAIL')
        self.live(LIVE + HISTORY.replace('</p>', ' Sley 1.2.1 is the current version.</p>'), 'FAIL')
        self.live(LIVE + '<p>The human-readable 1.x line lives at '
                  'GreyforgeLabs/sley-legacy (Sley 1.2.1, Apache-2.0).</p>')
        self.live(LIVE + HISTORY.replace('final human-readable release',
                  'final release of the complete human-readable line'))

    def test_live_stale_tag_does_not_hide_behind_another_current_link(self):
        self.live(LIVE.replace('/tag/v2.1.1', '/tag/v2.0.1', 1), 'FAIL')
        self.live(LIVE.replace('/tag/v2.1.1', '/tag/v2.1.0', 1) + HISTORY, 'FAIL')

    def test_live_wrong_target_with_current_label(self):
        self.live(LIVE.replace('/sley/releases', '/wrong/releases', 1), 'FAIL')
        self.live(LIVE.replace('/blob/v2.1.1/docs/QUICKSTART.md', '/blob/main/docs/QUICKSTART.md'), 'FAIL')
        self.live(LIVE.replace('SLEY-2.1.1.md', 'SLEY-2.0.1.md'), 'FAIL')

    def test_download_tag_and_asset_are_bound_together(self):
        download = c.REPO + '/releases/download/v2.1.1/'
        for asset in ('sley-2.1.1-linux-x86_64.tar.gz', 'SHA256SUMS'):
            self.live(LIVE + '<a href="' + download + asset + '">asset</a>')
        for asset in ('wrong.tar.gz', 'sley-2.0.1-linux-x86_64.tar.gz'):
            self.live(LIVE + '<a href="' + download + asset + '">asset</a>', 'FAIL')
        self.live(LIVE + '<a href="' + download.replace('v2.1.1', 'v2.0.1')
                  + 'sley-2.1.1-linux-x86_64.tar.gz">asset</a>', 'FAIL')

    def test_hidden_expected_text_does_not_count(self):
        self.live('<script>' + LIVE + '</script>', 'FAIL')
        self.live('<!--' + LIVE + '-->', 'FAIL')

    def test_plain_discovery_and_historical_selfhost(self):
        plain = '''Current release: Sley 2.1 (version 2.1.1)
Release page: https://github.com/sley-lang/sley/releases/tag/v2.1.1
Release archive: sley-2.1.1-linux-x86_64.tar.gz
Quickstart: https://github.com/sley-lang/sley/blob/v2.1.1/docs/QUICKSTART.md
Release notes: https://github.com/sley-lang/sley/blob/v2.1.1/docs/release/SLEY-2.1.1.md
Self-rebuild bundle: sley-2.1.0-selfhost-linux-x86_64.tar.gz from https://github.com/sley-lang/sley/releases/tag/v2.1.0
'''
        self.assertEqual(c.check_live('/llms.txt', plain, VERSION)['status'], 'PASS')
        self.assertEqual(c.check_live('/llms.txt', plain.replace('/tag/v2.1.1', '/tag/v2.0.1'), VERSION)['status'], 'FAIL')
        legacy = '\nSley 1.x Legacy - historical\n-------------------\nRelease: https://github.com/GreyforgeLabs/sley-legacy/releases/tag/v1.2.1\n'
        self.assertEqual(c.check_live('/README.txt', plain + legacy, VERSION)['status'], 'PASS')
        bad = plain.replace(c.REPO + '/releases/tag/v2.1.1', 'https://github.com/GreyforgeLabs/sley-legacy/releases/tag/v1.2.1')
        self.assertEqual(c.check_live('/README.txt', bad, VERSION)['status'], 'FAIL')
        self.assertEqual(c.check_live('/README.txt', bad + legacy, VERSION)['status'], 'FAIL')

    def test_text_markdown_links_use_their_labels(self):
        md = '''Current release: Sley 2.1 (version 2.1.1)
- [Sley 2.1.1 release](https://github.com/sley-lang/sley/releases/tag/v2.1.1): static binaries
- [sley-2.1.1-linux-x86_64.tar.gz](https://github.com/sley-lang/sley/releases/download/v2.1.1/sley-2.1.1-linux-x86_64.tar.gz): SHA-256 0
[Concepts](https://github.com/sley-lang/sley/blob/v2.1.1/docs/CONCEPTS.md) explains the model, and the [Quickstart](https://github.com/sley-lang/sley/blob/v2.1.1/docs/QUICKSTART.md) adds more.
- The word tier ran 2.2x faster ([2.0.0 release notes](https://github.com/sley-lang/sley/blob/v2.0.0/docs/release/SLEY-2.0.0.md#benchmarks)).
Self-rebuild bundle: sley-2.1.0-selfhost-linux-x86_64.tar.gz from the [2.1.0 release](https://github.com/sley-lang/sley/releases/tag/v2.1.0).
- [Self-hosting record](https://github.com/sley-lang/sley/blob/v2.1.0/docs/release/SH2_SELF_HOSTING.md)
- [2.1.1 release notes](https://github.com/sley-lang/sley/blob/v2.1.1/docs/release/SLEY-2.1.1.md)
'''
        for path in ('/llms.txt', '/README.txt'):
            self.assertEqual(c.check_live(path, md, VERSION)['status'], 'PASS', path)
        self.assertEqual(c.text_links('Quickstart: https://x.example/a. [b](https://x.example/b)'),
                         [('b', 'https://x.example/b'), ('Quickstart', 'https://x.example/a')])
        # A historical note still needs its exact "<version> release notes" label.
        for stale in ('[benchmarks](https://github.com/sley-lang/sley/blob/v2.0.0/docs/release/SLEY-2.0.0.md',
                      '[2.0.1 release notes](https://github.com/sley-lang/sley/blob/v2.0.0/docs/release/SLEY-2.0.0.md'):
            bad = md.replace('[2.0.0 release notes](https://github.com/sley-lang/sley/blob/v2.0.0/docs/release/SLEY-2.0.0.md', stale)
            self.assertEqual(c.check_live('/llms.txt', bad, VERSION)['status'], 'FAIL', stale)
        # A Markdown label does not launder a wrong primary destination.
        wrong = md.replace('blob/v2.1.1/docs/QUICKSTART.md', 'blob/v2.0.1/docs/QUICKSTART.md')
        self.assertEqual(c.check_live('/README.txt', wrong, VERSION)['status'], 'FAIL')
        old_bundle = md.replace('from the [2.1.0 release](https://github.com/sley-lang/sley/releases/tag/v2.1.0)',
                                'from the [old build](https://github.com/sley-lang/sley/releases/tag/v2.1.0)')
        self.assertEqual(c.check_live('/llms.txt', old_bundle, VERSION)['status'], 'FAIL')

    def test_default_never_fetches_and_uses_cargo_version(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'docs').mkdir()
            for name, text in [('README.md', README), ('docs/QUICKSTART.md', QUICKSTART), ('docs/README.md', INDEX)]:
                (root / name).write_text(text)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion="2.1.1"\n')
            with patch.object(c, 'fetch', side_effect=AssertionError('network forbidden')):
                self.assertEqual(c.run(root)['status'], 'PASS')
                (root / 'Cargo.toml').write_text('[workspace.package]\nversion="2.1.2"\n')
                self.assertEqual(c.run(root)['status'], 'FAIL')
            with patch.object(c, 'fetch', side_effect=OSError('unavailable')):
                report = c.run(root, live=True)
                self.assertEqual(sum('error' in s for s in report['surfaces']), 7)
                self.assertEqual(report['status'], 'FAIL')

    def test_cli_failure_is_json_and_nonzero(self):
        with patch.object(c, 'run', return_value={'status': 'FAIL'}), contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(c.main([]), 1)
        self.assertEqual(json.loads(output.getvalue())['status'], 'FAIL')

    def test_fetch_refuses_nonfixed_path_and_redirect(self):
        with self.assertRaises(ValueError):
            c.fetch('/llms-full.txt')
        with self.assertRaises(ValueError):
            c.NoRedirect().redirect_request(None, None, 302, '', {}, 'https://example.org')

    def test_fetch_size_type_and_read_deadline(self):
        class Response(io.BytesIO):
            status = 200
            headers = Message()
            headers['Content-Type'] = 'text/plain'

        for payload, clock in [(b'x' * (c.MAX_BYTES + 1), None), (b'ok', [0, 11])]:
            with patch.object(c.urllib.request, 'build_opener') as build:
                build.return_value.open.return_value = Response(payload)
                timer = patch.object(c.time, 'monotonic', side_effect=clock) if clock else contextlib.nullcontext()
                with timer, self.assertRaises(ValueError):
                    c.fetch('/llms.txt')
                self.assertEqual(build.return_value.open.call_args.kwargs['timeout'], 5)
        with patch.object(c.urllib.request, 'build_opener') as build:
            response = Response(b'not text')
            response.headers = Message()
            response.headers['Content-Type'] = 'application/octet-stream'
            build.return_value.open.return_value = response
            with self.assertRaises(ValueError):
                c.fetch('/')

    def test_fetch_rejects_truncated_content_length(self):
        payload = LIVE.encode()
        response = http_response(f'Content-Length: {len(payload) + 100}\r\n'.encode(), payload)
        with patch.object(c.urllib.request, 'build_opener') as build:
            build.return_value.open.return_value = response
            with self.assertRaisesRegex(ValueError, 'Content-Length'):
                c.fetch('/')

    def test_fetch_valid_content_length_chunked_and_eof(self):
        payload = LIVE.encode()
        chunked = f'{len(payload):X}\r\n'.encode() + payload + b'\r\n0\r\n\r\n'
        for headers, body in [(f'Content-Length: {len(payload)}\r\n'.encode(), payload),
                              (b'Transfer-Encoding: chunked\r\n', chunked), (b'', payload)]:
            with self.subTest(headers=headers), patch.object(c.urllib.request, 'build_opener') as build:
                build.return_value.open.return_value = http_response(headers, body)
                self.assertEqual(c.fetch('/'), LIVE)

    def test_fetch_rejects_invalid_or_oversized_framing(self):
        for headers in [b'Content-Length: invalid\r\n', b'Content-Length: -1\r\n',
                        b'Content-Length: 1\r\nContent-Length: 2\r\n',
                        b'Content-Length: 1, 2\r\n',
                        f'Content-Length: {c.MAX_BYTES + 1}\r\n'.encode(),
                        b'Content-Length: 1\r\nTransfer-Encoding: chunked\r\n']:
            with self.subTest(headers=headers), patch.object(c.urllib.request, 'build_opener') as build:
                build.return_value.open.return_value = http_response(headers, b'')
                with self.assertRaises(ValueError):
                    c.fetch('/')

    def test_incomplete_chunked_transfer_is_per_surface_json_failure(self):
        payload = LIVE.encode()
        incomplete = f'{len(payload):X}\r\n'.encode() + payload + b'\r\n'
        with patch.object(c.urllib.request, 'build_opener') as build:
            build.return_value.open.side_effect = lambda *args, **kwargs: http_response(
                b'Transfer-Encoding: chunked\r\n', incomplete)
            with contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(c.main(['--live-site']), 1)
            report = json.loads(output.getvalue())
            self.assertEqual(report['status'], 'FAIL')
            live = report['surfaces'][-len(c.LIVE_PATHS):]
            self.assertEqual([s['surface'] for s in live], [c.SITE + p for p in c.LIVE_PATHS])
            self.assertTrue(all(s['status'] == 'FAIL' and 'IncompleteRead' in s['error'] for s in live))


if __name__ == '__main__':
    unittest.main()
