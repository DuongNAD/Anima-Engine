# Security Policy (`MP-5.2`)

Per owner decision `QD-GATES-1127 (8)` (2026-11-27), Anima Engine accepts security reports through GitHub Private Vulnerability Reporting ([RELEASE_CHECKLIST.md §5](docs/how-to/RELEASE_CHECKLIST.md#5-stage-5-governance--mp-51-external-user-sessions), [CONTRIBUTING.md](CONTRIBUTING.md#reporting-a-security-issue)).

---

## Supported Versions

Anima Engine is pre-1.0 software. Security updates apply **only** to the latest pre-release / release candidate line (and, once tagged, the latest `0.9.x` release):

| Version | Supported | Notes |
|---|---|---|
| Latest `v0.9.0-rc.*` (upcoming) / `v0.9.x` | Yes (best effort) | Target `0.9` single-machine release line |
| Older pre-release snapshots / `0.1.0` prototypes | No | Update to the latest release candidate or tagged `0.9.x` build |

---

## Reporting a Vulnerability

- **Private channel (`QD-GATES-1127 (8)`):** Report a vulnerability privately through GitHub by opening the repository's **Security** tab and choosing **Report a vulnerability** at <https://github.com/DuongNAD/Anima-Engine/security/advisories/new>.
- **Do not open a public GitHub issue, discussion, or pull request** for a suspected security vulnerability.
- No personal email addresses are used for vulnerability intake.

When submitting a private report, please include as much of the following technical context as you can (without personal data or secrets):

1. Release version, tag, or Git commit SHA (and whether you used an official release/workflow artifact or a local source build).
2. Operating system, CPU architecture, and GPU/WebView runtime (for desktop builds).
3. Minimal reproduction steps, affected component/file path, and a proof-of-concept or sanitized log excerpt.
4. Estimated security or integrity impact.

Please allow maintainers reasonable time to investigate, reproduce, and release a fix before public disclosure.
