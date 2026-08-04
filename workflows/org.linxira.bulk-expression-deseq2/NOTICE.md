# Source and license notice

The workflow wrapper, schemas, tests, and documentation in this directory are
Copyright Linxira OS contributors and licensed `AGPL-3.0-or-later`.

Runtime dependencies are installed separately into an application-owned,
isolated environment and are not vendored in this pack:

| Component | Tested requirement | Canonical source | License signal |
| --- | --- | --- | --- |
| R | `>=4.6.1,<4.7.0`; 4.6.1 preferred | <https://cran.r-project.org/src/base/R-4/> | GPL-2.0-or-later OR GPL-3.0-or-later |
| DESeq2 | `>=1.52.0,<1.53.0` (Bioconductor 3.23) | <https://bioconductor.org/packages/3.23/bioc/html/DESeq2.html> | LGPL-3.0-or-later |
| jsonlite | `>=1.8.9,<3.0.0` | <https://cran.r-project.org/package=jsonlite> | MIT |
| digest | `>=0.6.37,<0.7.0` | <https://cran.r-project.org/package=digest> | GPL-2.0-or-later |

The implementation calls documented package APIs and does not copy upstream
source or examples. DESeq2 has a substantial Bioconductor dependency graph;
the future signed resolver must materialize and checksum its complete
transitive lock for the selected platform, R runtime, and project library
before this pack becomes installable. At execution, this pack rejects an
untested R minor version, incompatible declared-package versions, and any
declared package resolved outside the selected project library.
The result provenance also records the resolved version of every R namespace
loaded by the analysis, including transitive Bioconductor dependencies.

Scientific method citation:

Love MI, Huber W, Anders S. Moderated estimation of fold change and dispersion
for RNA-seq data with DESeq2. *Genome Biology*. 2014;15:550.
<https://doi.org/10.1186/s13059-014-0550-8>
