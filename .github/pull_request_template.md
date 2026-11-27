## Summary

<!-- Describe what this PR changes and why. Keep PRs small and focused on one logical change. -->

## Related Issues / Dependencies

<!-- Link issues (e.g., Closes #123) or upstream stacked PRs (e.g., Depends on #120). -->
<!-- Under Squash-Only merge: prefer Sequential PRs; if stacking, keep downstream PRs as Draft and restack via `git rebase --update-refs --onto origin/main <old-parent-tip> <top-branch>` (never plain rebase or GitHub "Update branch") after upstream merges. -->
- Closes #

## Type of Change

- [ ] `feat`: New feature
- [ ] `fix`: Bug fix
- [ ] `refactor`: Code refactoring (no behavioral change)
- [ ] `docs`: Documentation only
- [ ] `test`: Tests only
- [ ] `ci` / `build` / `chore`: Tooling, CI, or maintenance
- [ ] Breaking change (`!` in PR title and `BREAKING CHANGE:` footer below)

## Verification

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] PR title follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/)
- [ ] Documentation / tests updated where applicable

## AI-Assisted Disclosure (Optional)

<!-- If substantially authored by an AI coding agent, note the scope and confirm all changes were reviewed and verified. -->
- [ ] Human-reviewed and verified locally
