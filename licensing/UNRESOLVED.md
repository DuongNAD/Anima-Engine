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

One narrower route exists for exactly that case. A component that offers Apache-2.0 as a
whole operand of an SPDX `OR`, and whose artifact and upstream carry no licence file and no
notice at all, can be closed by **electing Apache-2.0** under `elections` in
[`upstream/sources.json`](upstream/sources.json): Apache-2.0 asks for its own text, which is
complete as published, plus the notices the work carries, and there are none, so nothing is
invented. MIT is never elected, because its notice condition points at a holder’s notice that
does not exist. Elections are listed at the end of this file. Whether a bare Cargo `license`
field is the author’s grant is a legal question this does not answer. This is a record of an
engineering decision, not legal advice.

**51 of the original 32 have been closed that way**, from 35 vendored file(s)
across 24 commit(s) in 21 repositories.

**0 component(s) unresolved.** Every distributed component supplies its
licence text.

## Closed by electing Apache-2.0

Recorded in [`upstream/sources.json`](upstream/sources.json) under `elections`, and
re-checked on every run: the election fails the run if the installed crate holds a
licence-like file or directory at any depth or a copyright notice in any file, or if the
pinned text stops being the Apache-2.0 copy the inventory ships most widely. The owner
authorised the choice on 2026-11-19 (QD-C2 in `docs/how-to/AGENT_WORKFLOW.md`).

### `utf8-chars` 3.0.7 — declares MIT OR Apache-2.0, elected Apache-2.0

Attribution: utf8-chars 3.0.7, by warlock <internalmike@gmail.com>, https://github.com/A1-Triard/utf8-chars. Searched at `https://github.com/A1-Triard/utf8-chars`
commit `9e35858ccf96914d4eb00c8d31be907e6487cbe4` (no release tag at that commit), 2026-11-19. Text `T0186` in
[`THIRD_PARTY_LICENSES.txt`](THIRD_PARTY_LICENSES.txt).

- The published .crate for utf8-chars@3.0.7 carries .cargo_vcs_info.json naming commit 9e35858ccf96914d4eb00c8d31be907e6487cbe4; git ls-remote --tags finds no tags in A1-Triard/utf8-chars.
- Cargo.toml at that commit declares name = "utf8-chars", version = "3.0.7", license = "MIT OR Apache-2.0".
- A recursive GitHub tree scan at commit 9e35858ccf96914d4eb00c8d31be907e6487cbe4 (default-branch HEAD, 5 blobs total) finds no LICENSE, LICENCE, COPYING, COPYRIGHT, NOTICE, or UNLICENSE file.
- README.md (364 bytes) and src/lib.rs contain no copyright notice or licence statement; no external template is substituted.
- Searched on 2026-11-18 and recorded then under `blocked`. Re-checked 2026-11-19 against the installed artifact: it holds Cargo.toml, Cargo.toml.orig, Cargo.lock, README.md, .gitignore, .cargo_vcs_info.json, .github/workflows/ci.yml and src/lib.rs, with no LICENSE, LICENCE, COPYING, COPYRIGHT, NOTICE or UNLICENSE file at any depth and no copyright line in any of them. The generator now repeats that scan on every run (scanElectedArtifact: licence-like names at any depth, and `copyright`, `(c) <year>` or the copyright sign in any file).
- Upstream was asked to publish licence files in A1-Triard/utf8-chars issue #4 and pull request #5, both opened 2026-11-18; neither had an answer on 2026-11-19. The crate stays in the shipped graph through pliron 0.19.0 (its `std` feature is `dep:utf8-chars`) under cubecl-core 0.11.4 (its `std` feature enables `pliron/std`), which burn brings in.
- Why MIT is not elected: MIT's text is a template whose notice condition points at "the above copyright notice". The crate has no holder's notice to reproduce, so an MIT text could only ship with no holder line or with an invented one, and scripts/gen_third_party_licenses.mjs never writes one.
- Why Apache-2.0 is elected: the crate is offered under `MIT OR Apache-2.0`, so the distributor may take either. Apache-2.0's text is complete as published, and section 4 asks (a) that recipients get a copy of the License; (b) that modified files carry prominent change notices, which does not apply because the crate is compiled unmodified; (c) that copyright, patent, trademark and attribution notices present in the Source form be retained, and there are none; (d) that a NOTICE file be carried if the Work includes one, and it does not. Electing Apache-2.0 meets those conditions with nothing synthesised.
- The elected text is not downloaded or kept as a private copy: it is the Apache-2.0 text the inventory already ships most widely, byte for byte, from the installed artifacts of other distributed crates (LICENSE-APACHE in ahash, arrayvec, async-channel and others: 101 installed copies when the election was made; the same bytes also appear as 5 vendored upstream copies for the unic-* crates, which the selection does not count). What is checked about it: the appendix placeholder line `Copyright [yyyy] [name of copyright owner]` is present and unfilled, and no line mentioning copyright, (c) or the copyright sign carries a year. Its SHA-256 is pinned in `textSha256` and re-checked on every run.
- Decision: the owner asked how the licence row could be cleared and authorised the means in chat on 2026-11-19 ("giấy phép có cách nào k, bạn có thể tự nhắn hỏi hoặc làm j tuỳ"); recorded as QD-C2 in docs/how-to/AGENT_WORKFLOW.md. This is an engineering record of a licence choice the declared expression permits, not legal advice.
- Open legal question, which the election does not answer: an election presupposes that the crate's bare Cargo `license` field is the author's grant. Whether it is, with no licence text or notice anywhere in the crate, is a legal question. The project treats the published crate's declared licence as the grant, as the Rust ecosystem does. Not legal advice.
