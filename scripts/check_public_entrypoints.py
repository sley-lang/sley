#!/usr/bin/env python3
"""Check current release entry points; offline unless --live-site is explicit."""
import argparse
import hashlib
from html.parser import HTMLParser
from http.client import HTTPException
import json
from pathlib import Path
import re
import time
import urllib.request

from release_version import ROOT, workspace_version

SITE = 'https://sleylang.org'
REPO = 'https://github.com/sley-lang/sley'
LIVE_PATHS = ('/', '/docs', '/tutorial', '/faq', '/readme', '/README.txt', '/llms.txt')
MAX_BYTES = 512 * 1024
TIMEOUT = 5
VERSION = r'\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?'
ARCHIVE = re.compile(r'sley-(?!2\.1\.0-selfhost-linux-x86_64)(' + VERSION + r')-linux-x86_64(?:\.tar\.gz)?')
URL = re.compile(r'https://[^\s<>"\[\]()]+')
MARKDOWN_LINK = re.compile(r'\[([^\]\n]+)\]\(([^\s)]+)\)')


class Page(HTMLParser):
    """Visible text and link labels; ignore scripts, styles and HTML comments."""
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts, self.links, self.images = [], [], []
        self.anchor, self.hidden = None, 0

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag in ('script', 'style'):
            self.hidden += 1
        if self.hidden:
            return
        if tag in ('p', 'div', 'li', 'section', 'h1', 'h2', 'h3', 'pre', 'br', 'tr'):
            self.parts.append('\n')
        if tag == 'a':
            self.anchor = (attrs.get('href', ''), len(self.parts))
        if tag == 'img':
            self.images.append(attrs)

    def handle_data(self, data):
        if not self.hidden:
            self.parts.append(re.sub(r'\s+', ' ', data))

    def handle_endtag(self, tag):
        if tag in ('script', 'style'):
            self.hidden = max(0, self.hidden - 1)
            return
        if self.hidden:
            return
        if tag == 'a' and self.anchor:
            href, start = self.anchor
            label = ''.join(self.parts[start:]).strip()
            self.links.append((label, href))
            self.parts.append(' [' + href + '] ')
            self.anchor = None
        if tag in ('p', 'div', 'li', 'section', 'h1', 'h2', 'h3', 'pre', 'tr'):
            self.parts.append('\n')


def result(name, body, checks):
    return {'surface': name, 'status': 'PASS' if all(c['pass'] for c in checks) else 'FAIL',
            'sha256': hashlib.sha256(body.encode()).hexdigest(), 'bytes': len(body.encode()),
            'checks': checks}


def check(checks, name, observed, expected, required=True):
    observed = list(observed)
    checks.append({'check': name, 'pass': (bool(observed) or not required)
                   and all(x == expected for x in observed),
                   'expected': expected, 'observed': observed})


def links(text):
    page = Page()
    page.feed(text)
    return page.links + MARKDOWN_LINK.findall(text)


def note_targets(text):
    return [url.split('#')[0] for _, url in links(text) if '/SLEY-' in url]


def current_prefix(text):
    return re.split(r'^## ', text, maxsplit=1, flags=re.M)[0]


def tagged_versions(text):
    return re.findall(re.escape(REPO) + r'/releases/(?:tag|download)/v(' + VERSION + r')', text)


def release_target(url, version, generic=False):
    return url in ({REPO + '/releases'} if generic else set()) | {
        REPO + '/releases/tag/v' + version,
        REPO + '/releases/download/v' + version + '/sley-' + version + '-linux-x86_64.tar.gz',
        REPO + '/releases/download/v' + version + '/SHA256SUMS'}


def local_release_links(checks, text, version):
    targets = [url for label, url in links(text) if label.lower() == 'releases']
    check(checks, 'canonical release destinations', [release_target(url, version, True) for url in targets], True)


def check_local(name, text, version):
    checks = []
    if name == 'README.md':
        page = Page()
        page.feed(text)
        badges = [image for image in page.images if '/badge/version-' in image.get('src', '')]
        check(checks, 'version badge label', [x.get('alt') for x in badges], 'Version ' + version)
        check(checks, 'version badge image', [re.sub(r'^.*?/badge/version-(.*?)-[0-9a-fA-F]{6}(?:\?.*)?$',
              r'\1', x['src']) for x in badges], version)
        check(checks, 'primary release-note links', note_targets(current_prefix(text)), 'docs/release/SLEY-' + version + '.md')
        check(checks, 'Linux archive/install names', ARCHIVE.findall(text), version)
        # Generic release landing pages are supported; pinned targets must agree.
        install = re.search(r'^## Quickstart\s*\n(.*?)(?=^## |\Z)', text, re.M | re.S)
        install = install.group(1) if install else ''
        check(checks, 'explicit release tags', tagged_versions(current_prefix(text) + install), version, False)
        local_release_links(checks, install, version)
    elif name == 'docs/QUICKSTART.md':
        install = re.search(r'^## 1\. Install\s*\n(.*?)(?=^## |\Z)', text, re.M | re.S)
        install = install.group(1) if install else ''
        check(checks, 'install version', re.findall(r'Sley (' + VERSION + r') supports', install), version)
        check(checks, 'Linux archive/install names', ARCHIVE.findall(text), version)
        check(checks, 'install release-note links', note_targets(install), 'release/SLEY-' + version + '.md')
        check(checks, 'explicit install release tags', tagged_versions(install), version, False)
        local_release_links(checks, install, version)
    else:
        prefix = current_prefix(text)
        check(checks, 'current release-note links', note_targets(prefix), 'release/SLEY-' + version + '.md')
        check(checks, 'current release label', re.findall(r'Current release:\s*\[Sley (' + VERSION + r')\]', prefix), version)
    return result(name, text, checks)


def text_links(line):
    """(label, url) for each https URL on one line of a text surface.

    A Markdown link `[label](url)` is labelled by its bracket text. Any other URL
    on the line keeps the `Label: url` reading: the text before the line's first
    colon, without a list marker.
    """
    found, spans = [], []
    for match in MARKDOWN_LINK.finditer(line):
        if URL.fullmatch(match.group(2)):
            found.append((match.group(1), match.group(2)))
            spans.append(match.span(2))
    prefix = line.split(':', 1)[0].lstrip('- ')
    for match in URL.finditer(line):
        if not any(start <= match.start() < end for start, end in spans):
            found.append((prefix, match.group().rstrip('.,;')))
    return found


def check_live(path, body, version):
    is_html = path not in ('/README.txt', '/llms.txt')
    page = Page()
    page.feed(body if is_html else '')
    visible = ''.join(page.parts) if is_html else body
    legacy = re.search(r'^Sley 1\.x Legacy[^\n]*\n-+\n(.*?)(?=^\S[^\n]*\n-+\n|\Z)',
                       body, re.M | re.S) if not is_html else None
    targets = [(label, url, False) for label, url in page.links] if is_html else [
        (label, url, bool(legacy and legacy.start(1) <= line.start() < legacy.end(1)))
        for line in re.finditer(r'^.*$', body, re.M) for label, url in text_links(line.group())]
    checks = []
    # All primary claims are checked, not just one occurrence of the expected version.
    claims = re.findall(r'\b(?:version\s+|Download Sley\s+|Install Sley\s+|Yes:\s*Sley\s+)('
                        + VERSION + r')', visible, re.I)
    for line in visible.splitlines():
        for match in re.finditer(r'\bSley (' + VERSION + r')(?:[,.]|\s+is\b)', line):
            # Exempt the identified legacy description, never every 1.x claim
            # in its paragraph or elsewhere on the page.
            legacy_description = re.match(r'Sley ' + VERSION + r' is the final '
                r'(?:human-readable release|release of the complete human-readable line)\b',
                line[match.start():])
            legacy_parenthesis = re.search(r'\bhuman-readable 1\.x line lives at '
                r'GreyforgeLabs/sley-legacy(?: \[[^\]\n]+\])?\s+\($', line[:match.start()])
            if not (match.group(1).startswith('1.') and 'GreyforgeLabs/sley-legacy' in line
                    and (legacy_description or legacy_parenthesis)):
                claims.append(match.group(1))
    claims += re.findall(r'Current release[: ]+(?:is )?(?:Sley )?(' + VERSION + r')', visible, re.I)
    claims += re.findall(r'(' + VERSION + r')\s+current release\b', visible, re.I)
    check(checks, 'current version claims', claims, version)
    check(checks, 'Linux archive/install names', ARCHIVE.findall(visible), version,
          path in ('/', '/docs', '/faq', '/README.txt', '/llms.txt'))
    releases, docs, notes, historical, primary = [], [], [], [], []
    for label, url, legacy_section in targets:
        label = label.strip()
        if (url.startswith('https://github.com/GreyforgeLabs/sley-legacy/')
                and (re.fullmatch(r'v1\.\d+\.\d+', label) or legacy_section)):
            historical.append(url)
            continue
        contexts = [line for line in visible.splitlines() if url in line]
        selfhost = (url == REPO + '/releases/tag/v2.1.0' and contexts
                    and all('sley-2.1.0-selfhost-linux-x86_64' in line for line in contexts)
                    and (label == '2.1.0 release' or label.lower() == 'self-rebuild bundle'))
        if selfhost:
            historical.append(url)
            continue
        if re.fullmatch(r'(?:Download Sley(?: ' + VERSION + r')?|(?:v?' + VERSION
                        + r' )?release(?: page)?|v' + VERSION + r')', label, re.I):
            primary.append(release_target(url, version))
        if re.fullmatch(r'(?:Sley )?(?:' + re.escape(version) + r' )?release notes', label, re.I):
            primary.append(url.split('#')[0] == REPO + '/blob/v' + version + '/docs/release/SLEY-' + version + '.md')
        if 'quickstart' in label.lower():
            primary.append(url.split('#')[0] in ('/docs', SITE + '/docs',
                           REPO + '/blob/v' + version + '/docs/QUICKSTART.md'))
        if not url.startswith(REPO + '/'):
            continue
        tag = re.fullmatch(re.escape(REPO) + r'/releases/(?:tag|download)/v(' + VERSION + r')(?:/([^?#]+))?(?:[?#].*)?', url)
        if tag:
            found, asset = tag.groups()
            releases.append(found)
            if asset:
                primary.append(release_target(url, version))
            continue
        doc = re.fullmatch(re.escape(REPO) + r'/blob/([^/]+)/docs/([^?#]+)(?:[?#].*)?', url)
        if doc:
            ref, filename = doc.groups()
            if ref == 'v2.1.0' and filename == 'release/SH2_SELF_HOSTING.md':
                historical.append(url)
                continue
            old_note = re.fullmatch(r'release/SLEY-(' + VERSION + r')\.md', filename)
            if (old_note and old_note.group(1) != version and ref == 'v' + old_note.group(1)
                    and label in (old_note.group(1) + ' release notes', 'Sley ' + old_note.group(1) + ' release notes')):
                historical.append(url)
                continue
            docs.append(ref)
            if filename.startswith('release/SLEY-'):
                notes.append(filename)
    check(checks, 'release tags', releases, version, path != '/tutorial')
    check(checks, 'current documentation refs', docs, 'v' + version)
    check(checks, 'current release-note targets', notes, 'release/SLEY-' + version + '.md', path != '/tutorial')
    check(checks, 'primary link destinations', primary, True)
    checks.append({'check': 'explicit historical links', 'pass': True, 'observed': historical})
    return result(SITE + path, body, checks)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError('redirect refused; only fixed canonical pages may be fetched')


def fetch(path):
    if path not in LIVE_PATHS:
        raise ValueError('not a fixed entry page')
    client = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    request = urllib.request.Request(SITE + path, headers={
        'User-Agent': 'Sley-public-entrypoints-check/1', 'Accept-Encoding': 'identity'})
    deadline = time.monotonic() + 10
    with client.open(request, timeout=TIMEOUT) as response:
        if response.status != 200 or response.headers.get_content_type() not in ('text/html', 'text/plain'):
            raise ValueError('expected HTTP 200 text response')
        if response.headers.get('Content-Encoding', 'identity') != 'identity':
            raise ValueError('compressed response refused')
        lengths = response.headers.get_all('Content-Length', [])
        transfers = response.headers.get_all('Transfer-Encoding', [])
        if transfers and (len(transfers) != 1 or transfers[0].strip().lower() != 'chunked' or lengths):
            raise ValueError('ambiguous or unsupported response framing')
        expected_size = None
        if lengths:
            if len(lengths) != 1 or not re.fullmatch(r'[0-9]+', lengths[0].strip()):
                raise ValueError('invalid or ambiguous Content-Length')
            expected_size = int(lengths[0])
            if expected_size > MAX_BYTES:
                raise ValueError('Content-Length exceeds response size limit')
        chunks, size = [], 0
        while True:
            chunk = response.read1(min(65536, MAX_BYTES + 1 - size))
            size += len(chunk)
            if size > MAX_BYTES or time.monotonic() > deadline:
                raise ValueError('response size or read deadline exceeded')
            if not chunk:
                break
            chunks.append(chunk)
        if expected_size is not None and size != expected_size:
            raise ValueError(f'incomplete response: Content-Length {expected_size}, received {size}')
    return b''.join(chunks).decode('utf-8')


def run(root=ROOT, live=False):
    version = workspace_version(root)
    surfaces = []
    for name in ('README.md', 'docs/QUICKSTART.md', 'docs/README.md'):
        try:
            with (root / name).open('rb') as stream:
                raw = stream.read(MAX_BYTES + 1)
            if len(raw) > MAX_BYTES:
                raise ValueError('input exceeds size limit')
            surfaces.append(check_local(name, raw.decode('utf-8'), version))
        except (OSError, ValueError) as exc:
            surfaces.append({'surface': name, 'status': 'FAIL', 'error': str(exc)})
    if live:
        for path in LIVE_PATHS:
            try:
                surfaces.append(check_live(path, fetch(path), version))
            except (OSError, ValueError, HTTPException) as exc:
                surfaces.append({'surface': SITE + path, 'status': 'FAIL', 'error': str(exc)})
    return {'schema': 'sley.public-entrypoints-check.v1', 'expected_version': version,
            'mode': 'source-and-live-site' if live else 'offline-source',
            'status': 'PASS' if all(s['status'] == 'PASS' for s in surfaces) else 'FAIL',
            'surfaces': surfaces}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--live-site', action='store_true', help='GET the seven fixed canonical site text pages')
    args = parser.parse_args(argv)
    try:
        report = run(live=args.live_site)
    except (OSError, ValueError, KeyError, TypeError) as exc:
        report = {'schema': 'sley.public-entrypoints-check.v1', 'status': 'FAIL', 'error': str(exc)}
    print(json.dumps(report, indent=2))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
