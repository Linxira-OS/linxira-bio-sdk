# org.linxira.batch-combat-py

ComBat batch-effect correction (parametric empirical Bayes) for expression
matrices, implemented in NumPy. Python backend of `expression.batch-correct.v1`;
the R backend `org.linxira.batch-combat-r` implements the identical
specification and the two are held to byte-identical TSV output.

Input: a features x samples expression matrix (CSV/TSV; first column = feature
id; use normalized or log-transformed values, not raw counts) and a sample
table whose first column holds sample ids matching the matrix columns, plus a
batch column (default name `batch`) and optional numeric or two-level
categorical covariates. Output: the corrected matrix (TSV) and a summary with
the empirical-Bayes priors.

Reference: Johnson, Li & Rabinovic, Biostatistics 8(1), 2007,
doi:10.1093/biostatistics/kxj037.
