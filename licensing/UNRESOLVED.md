# Unresolved third-party licence texts

**Generated — do not edit by hand.** Regenerate with
`node scripts/gen_third_party_licenses.mjs`.

Each component below is **distributed** inside the application and declares a licence,
but the artifact that was installed contains no copy of that licence text. Engineering
cannot close these by generating text: the canonical SPDX text of MIT contains no
copyright holder, and reproducing the holder’s notice is exactly what MIT requires. A
substituted text would look like compliance and would not be it.

Resolving one means obtaining the licence file from the upstream repository **at the
immutable commit the release was published from**, vendoring it under
[`upstream/`](upstream/) with its provenance in
[`upstream/sources.json`](upstream/sources.json), and re-running the generator. A row
survives here only when no such file exists upstream at all.

**51 of the original 32 have been closed that way**, from 35 vendored file(s)
across 24 commit(s) in 21 repositories.

**1 component(s) unresolved.** Distribution of the affected components
is blocked until each is closed.

| Component | Version | Ecosystem | Declared | Reason | Upstream |
|---|---|---|---|---|---|
| `utf8-chars` | 3.0.7 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/A1-Triard/utf8-chars |

## What was searched

Recorded in [`upstream/sources.json`](upstream/sources.json) under `blocked`, and
re-checked on every run: a component may not be listed there and resolved at the same
time.

### `utf8-chars` 3.0.7 — declares MIT OR Apache-2.0

Searched at `https://github.com/A1-Triard/utf8-chars` commit `9e35858ccf96914d4eb00c8d31be907e6487cbe4`
(no release tag at that commit), 2026-11-18.

- The published .crate for utf8-chars@3.0.7 carries .cargo_vcs_info.json naming commit 9e35858ccf96914d4eb00c8d31be907e6487cbe4; git ls-remote --tags finds no tags in A1-Triard/utf8-chars.
- Cargo.toml at that commit declares name = "utf8-chars", version = "3.0.7", license = "MIT OR Apache-2.0".
- A recursive GitHub tree scan at commit 9e35858ccf96914d4eb00c8d31be907e6487cbe4 (default-branch HEAD, 5 blobs total) finds no LICENSE, LICENCE, COPYING, COPYRIGHT, NOTICE, or UNLICENSE file.
- README.md (364 bytes) and src/lib.rs contain no copyright notice or licence statement; no external template is substituted.
