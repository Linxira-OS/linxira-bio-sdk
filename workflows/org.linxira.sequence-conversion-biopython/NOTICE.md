# Source and license notice

The workflow wrapper, schemas, tests, and documentation in this directory are
Copyright Linxira OS contributors and licensed `AGPL-3.0-or-later`.

Runtime dependencies are installed separately into an application-owned,
isolated environment and are not vendored in this pack:

| Component | Locked version | Source | License signal |
| --- | --- | --- | --- |
| Biopython | 1.85 | <https://github.com/biopython/biopython/tree/biopython-185> | Biopython License Agreement (permissive) |
| NumPy | 2.2.4 | <https://github.com/numpy/numpy/tree/v2.2.4> | BSD-3-Clause |

The implementation calls the public `Bio.SeqIO.convert` API; it does not copy
third-party source or examples. An installer must display and retain upstream
license texts and verify signed package artifacts before activation. The pack
remains `cataloged` because that installer is not implemented.
