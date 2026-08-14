# Bootstrap: install DESeq2 + jsonlite + digest into a project-isolated R library.
# Usage: Rscript scripts/bootstrap-deseq2-lib.R <library-dir>
args <- commandArgs(trailingOnly = TRUE)
lib <- normalizePath(args[[1]], mustWork = FALSE)
dir.create(lib, recursive = TRUE, showWarnings = FALSE)
.libPaths(c(lib, .libPaths()))
options(repos = c(CRAN = "https://cloud.r-project.org"))

if (!requireNamespace("BiocManager", quietly = TRUE)) {
  install.packages("BiocManager", lib = lib)
}
suppressMessages(BiocManager::install(
  c("jsonlite", "digest", "DESeq2"),
  lib = lib,
  update = FALSE,
  ask = FALSE,
  checkBuilt = TRUE
))

cat("INSTALLED\n")
for (pkg in c("BiocManager", "jsonlite", "digest", "DESeq2")) {
  if (requireNamespace(pkg, quietly = TRUE)) {
    cat(sprintf("%s %s\n", pkg, as.character(packageVersion(pkg, lib.loc = lib))))
  } else {
    cat(sprintf("%s MISSING\n", pkg))
  }
}
