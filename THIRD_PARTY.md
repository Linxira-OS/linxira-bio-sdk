# Third-Party Sources

This repository does not redistribute the ignored research clones under
`.research/`.

Project-owned components use `AGPL-3.0-or-later`. This does not replace or
relicense third-party code. Every dependency, adapted body, linked library,
model, database, and external executable must retain its own license and
notice. Incompatible dependencies must not enter a release merely because they
are technically convenient.

## Registered Research Sources

| Source | Root license signal | Current role |
| --- | --- | --- |
| `GPTomics/bioSkills` | MIT | Primary method and example review source |
| `BioTender-max/awesome-bio-agent-skills` | Collection index; nested terms vary | Discovery and classification source |
| `Linxira-OS/linxira-skills` | MIT | General skill platform and policy reference |

## Initial Runtime Components

| Component | License | Use |
| --- | --- | --- |
| `serde`, `serde_json` | MIT OR Apache-2.0 | Job, result, and capability serialization |
| `eframe`, `egui` | MIT OR Apache-2.0 | Native desktop GUI without an embedded WebView |
| `epaint_default_fonts` bundled fonts | OFL-1.1 AND Ubuntu-font-1.0 | Default native GUI fonts; retain the font license notices in desktop distributions |
| `hexf-parse` | CC0-1.0 | Transitive shader-number parsing used by the native GPU renderer |
| `uv` | Apache-2.0 OR MIT | Planned user-scoped Python runtime provider; not redistributed yet |
| `Pixi` | BSD-3-Clause | Planned mixed Python/R/Bioconda environment provider; not redistributed yet |
| `rig` | MIT | Planned R version manager; R itself retains GPL terms |
| Miniforge and Conda | BSD-3-Clause | Planned compatibility provider for Conda/Bioconda environments; not redistributed yet |
| Eclipse Temurin | GPL-2.0-only WITH Classpath-exception-2.0 | Planned Java 21/17 runtime provider; not redistributed yet |

Transitive Rust dependencies are locked in `Cargo.lock` and checked by
`cargo-deny` against `deny.toml`. Binary releases must include the generated
dependency and notice report.

Any adapted skill or implementation must record the exact source repository,
revision, path, license, retained notice, and a concrete modification summary.
An index entry is not permission to redistribute the indexed body.

## Dependency Gate

Before adding a dependency:

1. Record its SPDX identifier and source repository.
2. Confirm compatibility with `AGPL-3.0-or-later` and the intended linking or
   process boundary.
3. Prefer permissive MIT, Apache-2.0, BSD, ISC, Zlib, or similarly compatible
   components when capabilities are otherwise equivalent.
4. Review LGPL, MPL, GPL, model weights, database terms, SDK terms, and service
   terms individually.
5. Reject proprietary, source-available, field-of-use, non-commercial, or
   ambiguous terms from the default release unless a separate optional boundary
   and distribution decision is documented.
6. Generate a machine-readable dependency report and retain required notices in
   every binary and installer release.
