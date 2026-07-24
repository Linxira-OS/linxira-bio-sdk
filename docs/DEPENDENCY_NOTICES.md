# Dependency Notice Pipeline

Formal binary bundles must contain target-specific notices derived from the
actual locked Cargo release dependency closure. A successful `cargo-deny`
license audit is necessary but is not a substitute for retaining license and
notice text.

## Release Outputs

`scripts/stage-release.py` generates these files in every staging root:

- `THIRD_PARTY_DEPENDENCIES.json`: machine-readable packages, active features,
  source identifiers, Cargo VCS revisions, document hashes, retained texts,
  target triple, Cargo version, and `Cargo.lock` hash.
- `THIRD_PARTY_DEPENDENCIES.txt`: human-readable package index and complete
  retained texts for an About or Licenses screen and offline review.

The report has no timestamp or host path. The same release platform, target,
Cargo version, lock file, source archives, and override manifest therefore
produce byte-identical output.

Windows uses `x86_64-pc-windows-gnu`; Debian and Arch use
`x86_64-unknown-linux-gnu`. Dev-only dependencies are excluded, while normal
and build dependencies reachable from the three shipped binaries are retained.

The release also keeps the project `LICENSE`, policy-oriented `THIRD_PARTY.md`,
the pinned override manifest and source texts, and the separately bundled Noto
font license. Generated Cargo notices do not replace those files.

## Strict Failure Rules

Generation fails when a dependency has no license declaration, its complete
license text cannot be identified from normalized legal-text markers, a
compound SPDX expression has too few distinct retained documents, a packaged
notice is only an unresolved path pointer without an override, a package source
is an unreviewed path dependency, an override is stale, a Cargo VCS revision or
expression changed, or any pinned document hash does not match. New and
upgraded dependencies must be reviewed instead of silently inheriting an old
exception.

Some crates.io archives omit workspace-level license files even though their
Cargo metadata declares a license. Exact package version, registry source,
SPDX expression, Cargo VCS revision, upstream URL, reason, and document hash
for each such case are recorded in `licenses/cargo-overrides.json`. The
retained texts live under `licenses/cargo-overrides/`.

Canonical SPDX License List v3.28.0 fallbacks are explicit and fixed to an
exact commit. `hexf-parse 0.2.1` omits its CC0 text in both the crate and its
recorded upstream revision. `siphasher 1.0.3` retains attribution but omits the
full MIT and Apache texts. A small set of crates retain MIT or attribution but
ship only the short Apache reference; their complete Apache terms are added
without discarding the upstream notice. Every fallback is version- and
VCS-revision-specific and must be reviewed again on upgrade.

## Commands

Validate only the checked-in override configuration:

```bash
python scripts/generate_third_party_notices.py --check-config
python -m unittest discover -s tests/python -p "test_*.py"
```

After fetching the target's locked Cargo sources, generate without network
access:

```bash
cargo fetch --locked --target x86_64-pc-windows-gnu
python scripts/generate_third_party_notices.py --offline --platform windows --output build/notices/windows

cargo fetch --locked --target x86_64-unknown-linux-gnu
python scripts/generate_third_party_notices.py --offline --platform debian --output build/notices/debian
python scripts/generate_third_party_notices.py --offline --platform arch --output build/notices/arch
```

Release staging performs the same generation automatically and includes both
outputs in `bundle-manifest.lock.json`.
