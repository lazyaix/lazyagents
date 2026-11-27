## Summary

<!-- Describe what this PR changes and why. Keep PRs small and focused on one logical change. -->

## Related Issues / Dependencies

<!-- Link issues (e.g., Closes #123) or upstream stacked PRs (e.g., Depends on #120). -->
- Closes #

## Type of Change

- [ ] `feat`: New feature
- [ ] `fix`: Bug fix
- [ ] `refactor`: Code refactoring (no behavioral change)
- [ ] `docs`: Documentation only
- [ ] `test`: Tests only
- [ ] `ci` / `build` / `chore`: Tooling, CI, or maintenance
- [ ] Breaking change (`!` in PR title and `BREAKING CHANGE:` footer below)

<!-- lazyagents:start -->
<!-- Under squash-only merge: keep downstream stacked PRs as Draft and restack via `git rebase --update-refs --onto origin/main <old-parent-tip> <top-branch>` after upstream merges. -->

## Verification

- [ ] `cargo fmt --all -- --check` (Format)
- [ ] `CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features -- -D warnings` (Clippy)
- [ ] `cargo test --workspace --all-features` (Test)
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` (Docs)

## AI-Assisted Disclosure (Optional)

- [ ] Human-reviewed and verified locally
<!-- lazyagents:end -->
