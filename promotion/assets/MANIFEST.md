# Promotion asset manifest

Binaries do **not** live in git (see `../README.md`). This file records what each binary is,
its checksum, and where it actually is, so a video or a deck can be found again without
carrying it in the repository.

Rules:

- One row per produced artifact: screen recordings, edited videos, exported decks, cover
  images, thumbnail source files.
- `sha256` is of the stored file, not of an intermediate export.
- `location` names the storage (release attachment URL, object-storage path, or the local NAS
  path convention used by the maintainer), never an absolute developer path from a workstation.
- An artifact that is superseded stays listed with `superseded by <row>` instead of being
  deleted, so published links do not rot silently.

| artifact | piece | type | size | sha256 | location | recorded |
| --- | --- | --- | --- | --- | --- | --- |
| _(none yet)_ | — | — | — | — | — | — |

## Checklist before a piece is published

1. Script/outline text committed (`promotion/...`), with every number traceable to an archive
   path or a runnable command.
2. Binary stored, row added here with its checksum and location.
3. Platform description carries: machine block, input size, N, reproduction entry point,
   licence line.
4. After posting: the URL is backfilled into `../README.md`'s index.
