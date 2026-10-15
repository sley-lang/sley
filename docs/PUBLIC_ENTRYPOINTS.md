# Public entry-point consistency

Run the source check before preparing a release:

```sh
make public-entrypoints-check
# Equivalent: python3 scripts/check_public_entrypoints.py
```

The checker needs **Python 3.11 or newer**, with the standard library only.
It reads the expected workspace version from `Cargo.toml` through the existing
`scripts/release_version.py` helper. The default command never makes network
requests and does not run Sley, a compiler, or any fetched content.

The offline check covers three source entry points:

- `README.md`: the version badge, primary release-note link, Linux archive
  names, and the Quickstart's canonical release destination and pinned tags.
- `docs/QUICKSTART.md`: the install version, archive/unpack names, install
  release destination and tags, and matching release-note target.
- `docs/README.md`: the explicitly labeled current release and its note link
  before the historical guide index.

Generic links to the canonical repository's Releases landing page are allowed
in source installation instructions. Pinned tags must match the workspace
version. Primary local note targets must be the appropriate relative path to
`docs/release/SLEY-<version>.md`; fragments are allowed. Historical release
notes and self-hosting references outside the current introduction/install
sections keep their original identities. Protocol versions printed in the
Quickstart are separate from the workspace release version.

After deploying the website, run the explicit source-versus-site check:

```sh
make public-entrypoints-live-check
# Equivalent: python3 scripts/check_public_entrypoints.py --live-site
```

This also performs read-only GET requests to exactly seven canonical pages:
`https://sleylang.org/`, `/docs`, `/tutorial`, `/faq`, `/readme`, `/README.txt`,
and `/llms.txt`. It does not crawl or request linked GitHub assets, binaries,
scripts, images, or third-party pages. No browser, cookies, credentials, proxy settings
or provider APIs are used; redirects are refused.

Every recognized current version claim, primary release target, ordinary
Linux archive name, current documentation ref and release-note target must
agree with the source version. A correct value elsewhere cannot excuse a stale
primary link. Download links must pair the current tag with the exact Linux
archive or `SHA256SUMS`. HTML scripts, styles and comments do not supply evidence.
The historical 2.1.0 self-rebuild bundle and its explicitly contextualized release
link, the pinned self-hosting record, explicitly version-labeled historical note
links, and the separately labeled 1.x legacy lineage remain historical.

Responses must be UTF-8 HTML or plain text, uncompressed and at most 512 KiB.
Advertised Content-Length values must be valid, within that bound and fully
received. Ambiguous framing and incomplete transfers produce per-page FAIL;
complete chunked and no-length responses remain supported.
The request timeout is five seconds for socket inactivity. A ten-second elapsed
read deadline is checked after each read; it is **not a hard total network or
DNS deadline**. There are no retries. Use an outer process timeout when an
automation job needs a hard wall-clock limit.

Both commands emit JSON with the expected version, mode, per-surface PASS/FAIL,
content SHA-256, observed values and individual checks. Unavailable pages,
oversized responses and mismatches produce FAIL and exit status 1. A PASS checks
these entry-point contracts at observation time; it does not verify archive
bytes, linked-document availability, binary behavior, benchmark claims, or every
sentence/link on the website. Site-internal route tests remain separate.

Run the focused offline mutation regressions with:

```sh
python3 -B scripts/test_public_entrypoints.py
```

They cover contradictory versions/tags/archive names, wrong repositories and
note targets, tag/asset mismatches, historical exceptions, unavailable pages,
response bounds and framing, and the default command's no-network behavior.
