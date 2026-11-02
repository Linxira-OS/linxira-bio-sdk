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

Before submitting, replace `TO_BE_FILLED` with the SHA-256 of the source
tarball and replace the `recipe-maintainers` entry with the maintainer's
GitHub handle.

## Publication gate

Do not submit this recipe yet: `docs/proposals/schedule-2026-q4.md` defers
package publication until 2027-01-21. Retarget `version` and the source tag to
the release that contains the CLI `--help` and SDK packaging changes before
submitting.

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
