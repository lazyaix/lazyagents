# Security Policy

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues or discussions.**

Instead, use GitHub's **Private Vulnerability Reporting**:

- Open a private report at: <https://github.com/lazyaix/lazyagents/security/advisories/new>

Please include:
- A description of the vulnerability and its potential impact.
- Minimal reproduction steps or a proof of concept.
- Affected versions or commit SHAs, if known.

### Response Timeline

- **Acknowledgment**: Within 3 business days.
- **Triage & Assessment**: Within 7 business days.
- **Coordinated Disclosure**: Once a fix is prepared and validated, we will publish a GitHub Security Advisory and an immutable release containing the patch.

## Supply-Chain Security Baseline

`lazyaix/lazyagents` enforces the following GitHub-native supply-chain safeguards:

- All GitHub Actions workflows pin actions to full 40-character commit SHAs and declare least-privilege permissions (`permissions: {}` at top level).
- Continuous workflow auditing via `zizmor` and Rust dependency/advisory/license auditing via `cargo-deny` + Dependabot.
- Release tags (`v*`) are protected by immutable tag Rulesets and GitHub **Immutable Releases**.
