# Source and license notice

The benchmark harness, the independent capability implementations, schemas,
tests, and documentation in this directory are Copyright Linxira OS
contributors and licensed `AGPL-3.0-or-later`.

Runtime dependencies are resolved from the benchmark host's R installation (or
a project library named by `LINXIRA_BIO_WORKFLOW_R_LIBRARY`) and are not
vendored in this pack:

| Component | Requirement | Source | License signal |
| --- | --- | --- | --- |
| Biostrings | >=2.70.0,<3.0.0 | <https://bioconductor.org/packages/Biostrings/> | Artistic-2.0 |
| jsonlite | >=1.8.9,<3.0.0 | <https://cran.r-project.org/package=jsonlite> | MIT |
| digest | >=0.6.37,<0.7.0 | <https://cran.r-project.org/package=digest> | GPL-2.0-or-later |

The implementations call public package APIs only (`Biostrings::readBStringSet`,
`Biostrings::letterFrequency`, `Biostrings::width`); no third-party source or
example code is copied. The statistics definitions are restated from the
Linxira Bio Rust engine so the backends can be compared field by field.
