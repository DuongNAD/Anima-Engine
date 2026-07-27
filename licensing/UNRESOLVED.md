# Unresolved third-party licence texts

**Generated — do not edit by hand.** Regenerate with
`node scripts/gen_third_party_licenses.mjs`.

Each component below is **distributed** inside the application and declares a licence,
but the artifact that was installed contains no copy of that licence text. Engineering
cannot close these by generating text: the canonical SPDX text of MIT contains no
copyright holder, and reproducing the holder’s notice is exactly what MIT requires. A
substituted text would look like compliance and would not be it.

Resolving one means obtaining the licence file from the upstream repository **at the tag
matching the version below**, recording where it came from, and re-running the
generator. That is a human decision about a legal artifact; this file names the work,
it does not do it.

**32 component(s) unresolved.** Distribution of the affected components
is blocked until each is closed.

| Component | Version | Ecosystem | Declared | Reason | Upstream |
|---|---|---|---|---|---|
| `alloc-stdlib` | 0.2.4 | cargo | BSD-3-Clause | declares BSD-3-Clause but the installed artifact contains no licence file | https://github.com/dropbox/rust-alloc-no-stdlib |
| `codespan-reporting` | 0.11.1 | cargo | Apache-2.0 | declares Apache-2.0 but the installed artifact contains no licence file | https://github.com/brendanzab/codespan |
| `com_macros` | 0.6.0 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/microsoft/com-rs |
| `com_macros_support` | 0.6.0 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/microsoft/com-rs |
| `d3d12` | 0.19.0 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/gfx-rs/wgpu/tree/trunk/d3d12 |
| `delegate` | 0.10.0 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/kobzol/rust-delegate |
| `gpu-alloc-types` | 0.3.1 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/zakarumych/gpu-alloc |
| `gpu-alloc` | 0.6.2 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/zakarumych/gpu-alloc |
| `gpu-descriptor-types` | 0.1.2 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/zakarumych/gpu-descriptor |
| `gpu-descriptor` | 0.2.4 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/zakarumych/gpu-descriptor |
| `hexf-parse` | 0.2.1 | cargo | CC0-1.0 | declares CC0-1.0 but the installed artifact contains no licence file | https://github.com/lifthrasiir/hexf |
| `naga` | 0.19.2 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/gfx-rs/wgpu/tree/trunk/naga |
| `neo4rs-macros` | 0.3.0 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/neo4j-labs/neo4rs |
| `neo4rs` | 0.7.3 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/neo4j-labs/neo4rs |
| `profiling` | 1.0.18 | cargo | MIT OR Apache-2.0 | declares MIT OR Apache-2.0 but the installed artifact contains no licence file | https://github.com/aclysma/profiling |
| `pulp-wasm-simd-flag` | 0.1.1 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/sarah-quinones/pulp/ |
| `selectors` | 0.36.1 | cargo | MPL-2.0 | declares MPL-2.0 but the installed artifact contains no licence file | https://github.com/servo/stylo |
| `spirv` | 0.3.0+sdk-1.3.268.0 | cargo | Apache-2.0 | declares Apache-2.0 but the installed artifact contains no licence file | https://github.com/gfx-rs/rspirv |
| `ts-rs-macros` | 10.1.0 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/Aleph-Alpha/ts-rs |
| `ts-rs` | 10.1.0 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/Aleph-Alpha/ts-rs |
| `unic-char-property` | 0.9.0 | cargo | MIT/Apache-2.0 | declares MIT/Apache-2.0 but the installed artifact contains no licence file | https://github.com/open-i18n/rust-unic/ |
| `unic-char-range` | 0.9.0 | cargo | MIT/Apache-2.0 | declares MIT/Apache-2.0 but the installed artifact contains no licence file | https://github.com/open-i18n/rust-unic/ |
| `unic-common` | 0.9.0 | cargo | MIT/Apache-2.0 | declares MIT/Apache-2.0 but the installed artifact contains no licence file | https://github.com/open-i18n/rust-unic/ |
| `unic-ucd-ident` | 0.9.0 | cargo | MIT/Apache-2.0 | declares MIT/Apache-2.0 but the installed artifact contains no licence file | https://github.com/open-i18n/rust-unic/ |
| `unic-ucd-version` | 0.9.0 | cargo | MIT/Apache-2.0 | declares MIT/Apache-2.0 but the installed artifact contains no licence file | https://github.com/open-i18n/rust-unic/ |
| `webview2-com-macros` | 0.8.1 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/wravery/webview2-rs |
| `webview2-com-sys` | 0.38.2 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/wravery/webview2-rs |
| `webview2-com` | 0.38.2 | cargo | MIT | declares MIT but the installed artifact contains no licence file | https://github.com/wravery/webview2-rs |
| `zune-inflate` | 0.2.54 | cargo | MIT OR Apache-2.0 OR Zlib | declares MIT OR Apache-2.0 OR Zlib but the installed artifact contains no licence file | _none declared_ |
| `@oxc-project/runtime` | 0.139.0 | npm | _none_ | declares no licence in its own manifest; is compiled into the output by the toolchain and is not present in the install tree | _none declared_ |
| `@pixi/colord` | 2.9.6 | npm | MIT | declares MIT but the installed artifact contains no licence file | omgovich/colord |
| `@react-three/fiber` | 8.18.0 | npm | MIT | declares MIT but the installed artifact contains no licence file | git+https://github.com/pmndrs/react-three-fiber.git |
