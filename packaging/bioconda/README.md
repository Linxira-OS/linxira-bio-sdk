# Bioconda recipe

This directory is the source recipe for the `linxira-bio` Bioconda package.
It is not part of the release bundle. To submit it, copy the directory to
`bioconda-recipes/recipes/linxira-bio` and open a pull request.

The recipe builds only the headless surfaces:

- `linxira-bio` (CLI)
- `linxira-bio-mcp` (stdio MCP server)
- `linxira-bio-worker` (shared job executor)

The native UI stays in the GitHub release archives. The recipe is initially
limited to `linux-64`; macOS and aarch64 support need their own CI evidence.

The build installs `workflows/` and `capabilities/` under
`$PREFIX/share/linxira-bio` and sets `LINXIRA_BIO_WORKFLOW_ROOT` on activation
so `linxira-bio workflow run` can find the bundled packs.

`cargo-bundle-licenses` scans the full Cargo workspace, including UI-only
crates that are not installed by this package. `THIRDPARTY.yml` is therefore
broader than the headless payload; a future split workspace can narrow it.

The checked-in recipe is pinned to the `v1.1.5` release-asset tarball
`linxira-bio-sdk-1.1.5.tar.gz` and its SHA-256. When retargeting, replace both
the version and the checksum, and keep `recipe-maintainers` set to the
maintainer's GitHub handle.

## Publication gate

The 2027-01-21 package-publication gate was overridden on 2026-11-02 for an
adoption trial, recorded in `docs/proposals/schedule-2026-q4.md`. This recipe
targets `v1.1.5`; submit it through a `bioconda-recipes` pull request only
after the release asset exists and the source-repository CI is green.

## Local validation

Build inside the Linux filesystem (for example `/root/linxira-work`), not on a
Windows mount. Rust and conda caches on `/mnt/c` are slow and can be cleaned
between sessions.

```bash
export MAMBA_ROOT_PREFIX=/root/linxira-work/mamba
/root/linxira-work/tools/bin/micromamba create \
  -p /root/linxira-work/conda -c conda-forge conda-build
```

Use a local recipe copy with `source: path: /root/linxira-work/src/linxira-bio-sdk`
so validation exercises the working tree before a release tag exists, then run:

```bash
/root/linxira-work/conda/bin/conda-build /root/linxira-work/local-recipe \
  --croot /root/linxira-work/conda-bld \
  -c conda-forge -c bioconda

/root/linxira-work/conda/bin/conda index /root/linxira-work/conda-bld
/root/linxira-work/conda/bin/conda create -p /root/linxira-work/test-env \
  -c file:///root/linxira-work/conda-bld -c conda-forge -c bioconda linxira-bio
```
