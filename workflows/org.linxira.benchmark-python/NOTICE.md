# Source and license notice

The benchmark harness, the independent capability implementations, schemas,
tests, and documentation in this directory are Copyright Linxira OS
contributors and licensed `AGPL-3.0-or-later`.

Runtime dependencies are installed separately into an application-owned,
isolated environment and are not vendored in this pack:

| Component | Locked version | Source | License signal |
| --- | --- | --- | --- |
| Biopython | 1.85 | <https://github.com/biopython/biopython/tree/biopython-185> | Biopython License Agreement (permissive) |
| NumPy | 2.2.4 | <https://github.com/numpy/numpy/tree/v2.2.4> | BSD-3-Clause |

The implementations call public library APIs only (`Bio.SeqIO.FastaIO.
SimpleFastaParser`); no third-party source or example code is copied. The
statistics definitions are restated from the Linxira Bio Rust engine so the
two backends can be compared field by field.
