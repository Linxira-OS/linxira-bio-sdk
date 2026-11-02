# Versioning policy

## One product version

`X.Y.Z` identifies the product release train. The same version is used by:

- the native binaries and the GitHub release tag `vX.Y.Z`;
- the conda package `linxira-bio`;
- the PyPI distribution `linxira-bio-sdk`.

The SDK is a Python client for the same product, not a second engine. Its
distribution version follows the product version even when its Python source
did not change. A release that needs a different SDK version is an explicit
exception and must add a compatibility row to the release notes.

Capability ids keep their own contract version, such as
`sequence.stats.v1`. Those versions are independent of `X.Y.Z` and are never
derived from the product version.

The conda `build: number` and build string are packaging revisions, not product
versions. They may change while `X.Y.Z` stays fixed.

## Current state

| Release | Native / conda / GitHub | PyPI SDK | Note |
| --- | --- | --- | --- |
| `v1.1.5` | `1.1.5` | `0.1.0` | PyPI release predates this policy; `0.1.0` is immutable |

The next SDK publication uses the then-current product version (for example
`1.1.6`), after which every channel stays in lockstep.

## Bumping a release

1. Bump the workspace version in `Cargo.toml`.
2. Set the Python SDK distribution metadata and the Bioconda recipe to the
   same version.
3. Tag `vX.Y.Z`.
4. Run the version-equality check before publishing; CI runs it on every push
   and release tag. A mismatch must fail the release rather than produce
   another mapping row.

## Why not independent versions

Independent SDK semver is appropriate for a library with its own API surface.
This SDK is a thin subprocess client over one product contract. A second
version axis would make support and release notes ambiguous, and the
compatibility matrix would need maintenance without adding information.
