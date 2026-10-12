#!/usr/bin/env Rscript
# Ecosystem-tool arm for `expression.pca.v1`: the call a user would actually make.
#
# The parity arm in this repository hand-writes the power iteration, because the point of
# a parity arm is to isolate the language runtime. This script exists to answer the other
# question - "what would I get if I just used R?" - so it does the idiomatic thing:
# read the matrix, transpose so samples are rows, and call `prcomp` with centring only
# (the same mathematical operation the parity arm performs by hand).
#
# Invocation is recorded verbatim in the report (`tool.invocation`), so a disagreement
# between this and the parity arm can be attributed to the call, not guessed.

arguments <- commandArgs(trailingOnly = TRUE)
if (length(arguments) != 1L) {
  stop("usage: r-prcomp.R <matrix.tsv|matrix.csv[.gz]>", call. = FALSE)
}
path <- arguments[[1L]]

read_matrix <- function(path) {
  connection <- if (grepl("\\.gz$", path)) gzfile(path, "rt") else file(path, "rt")
  lines <- tryCatch(readLines(connection, warn = FALSE), finally = close(connection))
  nonblank <- lines[nzchar(trimws(lines))]
  if (length(nonblank) == 0L) stop("expression matrix is empty", call. = FALSE)
  delimiter <- if (grepl("\t", nonblank[[1L]], fixed = TRUE)) "\t" else ","
  table <- read.delim(
    text = paste(nonblank, collapse = "\n"),
    header = TRUE,
    sep = delimiter,
    row.names = 1,
    check.names = FALSE,
    quote = "",
    comment.char = ""
  )
  as.matrix(table)
}

features_by_samples <- read_matrix(path)
if (nrow(features_by_samples) < 1L || ncol(features_by_samples) < 2L) {
  stop("expected a feature identifier column and at least two samples", call. = FALSE)
}
if (anyNA(features_by_samples)) {
  stop("matrix contains missing values; impute or filter before this analysis", call. = FALSE)
}

samples_by_features <- t(features_by_samples)
component_limit <- min(ncol(samples_by_features), nrow(samples_by_features) - 1L)
result <- prcomp(samples_by_features, center = TRUE, scale. = FALSE, rank. = component_limit)

variance <- result$sdev^2
cat(sprintf(
  "prcomp samples=%d features=%d components=%d variance_explained=%.6f\n",
  nrow(samples_by_features),
  ncol(samples_by_features),
  length(result$sdev),
  sum(variance) / sum(apply(samples_by_features, 2L, var)) 
))
