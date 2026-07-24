# Source and license notice

The workflow wrapper, schemas, tests, and documentation in this directory are
Copyright Linxira OS contributors and licensed `AGPL-3.0-or-later`.

Runtime dependencies are installed separately into an application-owned,
isolated environment and are not vendored in this pack:

| Component | Locked version | Canonical source | License signal |
| --- | --- | --- | --- |
| R | 4.4.3 | <https://cran.r-project.org/src/base/R-4/R-4.4.3.tar.gz> | GPL-2.0-or-later OR GPL-3.0-or-later |
| DESeq2 | 1.46.0 (Bioconductor 3.20) | <https://bioconductor.org/packages/3.20/bioc/src/contrib/DESeq2_1.46.0.tar.gz> | LGPL-3.0-or-later |
| jsonlite | 1.8.9 | <https://cloud.r-project.org/src/contrib/Archive/jsonlite/jsonlite_1.8.9.tar.gz> | MIT |
| digest | 0.6.37 | <https://cloud.r-project.org/src/contrib/Archive/digest/digest_0.6.37.tar.gz> | GPL-2.0-or-later |

The implementation calls documented package APIs and does not copy upstream
source or examples. DESeq2 has a substantial Bioconductor dependency graph;
the future signed resolver must materialize and checksum its complete
transitive lock before this pack becomes installable. At execution, this pack
rejects drift in R and each directly invoked package version.
The result provenance also records the resolved version of every R namespace
loaded by the analysis, including transitive Bioconductor dependencies.

Scientific method citation:

Love MI, Huber W, Anders S. Moderated estimation of fold change and dispersion
for RNA-seq data with DESeq2. *Genome Biology*. 2014;15:550.
<https://doi.org/10.1186/s13059-014-0550-8>
