## What this changes

<!-- One paragraph. Link the issue if there is one. -->

## Gates

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `python scripts/validate-repository.py`
- [ ] pack suite + `python scripts/update-pack-manifest.py workflows/<pack>` (if a pack changed)

## Contribution terms (see CONTRIBUTING.md)

- [ ] Commits are signed off (`git commit -s`, DCO 1.1)

- [ ] One of the two applies (tick and complete the sentence):

  - [ ] I license my contribution under AGPL-3.0-or-later **and** grant the maintainers
        the right to also license it under the project's commercial terms. — Name: `<your name>`
  - [ ] I cannot grant the commercial relicensing right; please take this under
        AGPL-3.0-or-later only and record that in this pull request.

<!-- The second option is legitimate. It limits the dual-license offer, and saying so
     up front is better than discovering it later. -->
