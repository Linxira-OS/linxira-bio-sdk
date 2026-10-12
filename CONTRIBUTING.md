# Contributing / 贡献指南

> This project is dual-licensed: `AGPL-3.0-or-later` for everyone, plus separate
> commercial terms for organisations that cannot accept the AGPL (see `COMMERCIAL.md`).
> That second option only exists while the maintainers hold the rights to license every
> line of the code both ways. The contribution terms below are what keeps that possible.

## Before you write code / 动手之前

- Read `AGENTS.md`: it is the routing and rules file for this repository (capability
  registration in `ROADMAP.md` §11 before implementing, determinism requirements, the
  `.research/` airgap, execution-safety rules).
- Behaviour changes need a test that fails before and passes after. Tests that pin
  incidental wording or implementation details are not accepted; tests that catch a
  plausible user-visible bug are.
- Every number that lands in `docs/` or `benchmark-results/` must be reproducible from
  a command in this repository, and every figure is verified against its source data.

## Gates / 门禁

Run these before opening a pull request (Linux filesystem — not a shared folder or
`/mnt/c`):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python scripts/validate-repository.py
```

If your change touches a pack under `workflows/`, also run that pack's suite and
`python scripts/update-pack-manifest.py workflows/<pack>`: the manifest hashes are
validated, so a stale one fails CI.

## Contribution terms / 贡献条款 — read this before opening a pull request

1. **Sign off your commits** (`git commit -s`). The sign-off is the Developer
   Certificate of Origin 1.1: you wrote the change, or you have the right to submit it,
   and you understand it is public. 每个提交都要 `-s` 签名（DCO 1.1）。

2. **Grant a relicensing right.** A sign-off certifies provenance; it does **not** by
   itself allow the maintainers to offer your work commercially. So a pull request also
   needs this sentence in its description, with your name filled in:

   > I license my contribution to this repository under AGPL-3.0-or-later, and I grant
   > the maintainers the right to also license it under the project's commercial terms.

   You keep the copyright to your work; you are granting a right to relicense it, not
   assigning ownership. 你保留著作权，授予的是"可另按商业条款授权"的权利。

3. **If you cannot grant that**, say so in the pull request. The maintainers will decide
   whether to take the work under AGPL-3.0-or-later only, and that decision is recorded
   in the pull request. We would rather have the fix with a documented limitation than
   pretend the dual-license offer applies to code it does not cover.

Why this is asked at all: without an explicit grant, a single merged patch from a
contributor who only agreed to the AGPL makes it impossible to license the affected code
commercially — not because anyone objected, but because the rights were never granted.
The grant above is the smallest thing that avoids that dead end.

## Reporting instead of coding / 只报告问题也可以

Bug reports, reproduction attempts and criticism of the published benchmarks are as
valuable as patches here — `REPRODUCE.md` explains what a reviewer needs, and if a claim
in an article does not match the archive, that is the highest-value issue you can open.
