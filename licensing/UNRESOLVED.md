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

**31 of the original 32 have been closed that way**, from 39 vendored file(s)
across 24 commit(s) in 19 repositories.

**1 component(s) unresolved.** Distribution of the affected components
is blocked until each is closed.

| Component | Version | Ecosystem | Declared | Reason | Upstream |
|---|---|---|---|---|---|
| `hexf-parse` | 0.2.1 | cargo | CC0-1.0 | declares CC0-1.0 but the installed artifact contains no licence file | https://github.com/lifthrasiir/hexf |

## What was searched

Recorded in [`upstream/sources.json`](upstream/sources.json) under `blocked`, and
re-checked on every run: a component may not be listed there and resolved at the same
time.

### `hexf-parse` 0.2.1 — declares CC0-1.0

Searched at `https://github.com/lifthrasiir/hexf` commit `4225763d744183d720f575ae96d04161b4d08ea0`
(tag `0.2.1`), 2026-07-27.

- The published .crate carries .cargo_vcs_info.json naming commit 4225763d744183d720f575ae96d04161b4d08ea0, and git ls-remote resolves the tag 0.2.1 to that same commit. parse/Cargo.toml there declares name = "hexf-parse", version = "0.2.1", license = "CC0-1.0". The revision is not in doubt; the text is.
- A recursive tree scan of the repository at that commit finds no path matching LICEN[CS]E, COPYING, COPYRIGHT, NOTICE or UNLICENSE anywhere — not at the root, not in parse/. The only match on the word is tests/example.rs.
- README.md at that commit contains no licence section: a case-insensitive search for "licen", "public domain" and "cc0" returns nothing.
- The only LICENSE ever committed to the repository arrived in 41f0018229c1ee3d6fd813b6808d1ad1f506554c on 2024-12-04, three and a half years after this release, and its text is the Zero-Clause BSD permission grant ("Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted") — a different licence from the CC0-1.0 this version declares. Vendoring it would attribute the wrong terms to the shipped code.
- crates.io records license = "CC0-1.0" for all three published versions (0.1.0, 0.2.0, 0.2.1), so the declaration is consistent and the later file is a relicensing rather than a correction to this one.
- No CC0-1.0 text is substituted from a licence list. CC0 imposes no attribution condition, so the omission carries no notice obligation — but engineering does not get to package a text this project has never published and record it as that project’s licence file. Closing this row is a legal decision about whether the crates.io declaration alone suffices, not an engineering one.
