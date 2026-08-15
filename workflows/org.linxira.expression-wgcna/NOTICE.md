# Source and license notice

The workflow wrapper, schemas, tests, and documentation in this directory are
Copyright Linxira OS contributors and licensed `AGPL-3.0-or-later`.

Runtime dependencies are installed separately into an application-owned,
isolated environment and are not vendored in this pack:

| Component | Tested requirement | Canonical source | License signal |
| --- | --- | --- | --- |
| R | `>=4.6.1,<4.7.0`; 4.6.1 preferred | <https://cran.r-project.org/src/base/R-4/> | GPL-2.0-or-later OR GPL-3.0-or-later |
| WGCNA | `>=1.72,<2.0` (CRAN) | <https://cran.r-project.org/package=WGCNA> | GPL-2.0-or-later |
| jsonlite | `>=1.8.9,<3.0.0` | <https://cran.r-project.org/package=jsonlite> | MIT |
| digest | `>=0.6.37,<0.7.0` | <https://cran.r-project.org/package=digest> | GPL-2.0-or-later |

The implementation calls documented package APIs and does not copy upstream
source or examples. WGCNA has a substantial Bioconductor dependency graph;
the signed resolver must materialize and checksum its complete transitive
lock for the selected platform, R runtime, and project library before this
pack becomes installable. At execution, this pack rejects an untested R minor
version, incompatible declared-package versions, and any declared package
resolved outside the selected project library. The result provenance also
records the resolved version of every R namespace loaded by the analysis,
including transitive Bioconductor dependencies.

Scientific method citation:

Langfelder P, Horvath S. WGCNA: an R package for weighted correlation network
analysis. *BMC Bioinformatics*. 2008;9:559.
<https://doi.org/10.1186/1471-2105-9-559>
