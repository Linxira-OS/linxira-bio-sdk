test_file <- sub(
  "^--file=", "",
  commandArgs(trailingOnly = FALSE)[grep("^--file=", commandArgs(trailingOnly = FALSE))]
)
script <- normalizePath(
  file.path(dirname(test_file), "..", "src", "run_combat.R"),
  winslash = "/", mustWork = TRUE
)
source(script, local = TRUE)

root <- tempfile("linxira-combat-validation-")
dir.create(root)
on.exit(unlink(root, recursive = TRUE, force = TRUE), add = TRUE)

# deterministic synthetic data: 200 features x 12 samples, 2 batches x 6,
# a biological group effect that must survive correction, and a batch shift
# that must be removed. Values are log2-scale expression.
set.seed(20260202)
n_features <- 200L
base <- rlnorm(n_features, meanlog = 5, sdlog = 1)
group_effect <- rnorm(n_features, 0, 0.6)
samples <- paste0("S", 0:11)
batch <- c(rep("B0", 6), rep("B1", 6))
condition <- rep(c("control", "case"), 6)
libsize <- round(runif(12, 0.9, 1.1), 4)
values <- matrix(0, nrow = n_features, ncol = 12)
for (j in seq_len(12)) {
  shift <- if (batch[j] == "B0") 0.8 else -0.5
  lambda <- base * libsize[j] * exp(group_effect * (condition[j] == "case") + shift)
  values[, j] <- log2(rpois(n_features, lambda) + 1)
}
matrix_path <- file.path(root, "matrix.csv")
lines <- c(paste0("feature,", paste(samples, collapse = ",")))
for (i in seq_len(n_features)) {
  lines <- c(lines, paste0("G", sprintf("%04d", i - 1L), ",",
    paste(sprintf("%.6f", values[i, ]), collapse = ",")))
}
writeLines(lines, matrix_path)
sample_path <- file.path(root, "samples.csv")
writeLines(c("sample,batch,condition,libsize",
  paste0(samples, ",", batch, ",", condition, ",", libsize)), sample_path)

make_request <- function(out_dir, filename) {
  request_path <- file.path(root, paste0("request-", filename))
  write(toJSON(list(
    schema_version = "2",
    job_id = "combat-r-validation",
    capability = "expression.batch-correct.v1",
    inputs = list(
      list(artifact_id = "matrix", role = "expression-matrix", cardinality = "single",
        files = list(list(file_id = "m1", path = matrix_path, format = "csv",
          compression = "none", size_bytes = file.info(matrix_path)$size))),
      list(artifact_id = "samples", role = "sample-metadata", cardinality = "single",
        files = list(list(file_id = "s1", path = sample_path, format = "csv",
          compression = "none", size_bytes = file.info(sample_path)$size)))
    ),
    execution = list(mode = "local-cpu"),
    parameters = list(output_directory = out_dir, output_filename = filename,
      batch_column = "batch", method = "combat")
  ), auto_unbox = TRUE, digits = NA, null = "null"), request_path)
  request_path
}

run_ok <- function(out_dir, filename) {
  request_path <- make_request(out_dir, filename)
  result_path <- file.path(root, paste0("result-", filename))
  code <- main(list("--request", request_path, "--result", result_path))
  list(code = code, result = fromJSON(result_path, simplifyVector = FALSE))
}

# --- happy path ---
out1 <- file.path(root, "out1")
dir.create(out1)
run <- run_ok(out1, "corrected.tsv")
stopifnot(identical(run$code, 0L))
stopifnot(identical(run$result$status, "ok"))
stopifnot(identical(run$result$result$method, "combat-parametric"))
stopifnot(identical(run$result$result$feature_count, n_features))
stopifnot(identical(run$result$result$batch_counts$B0, 6L),
          identical(run$result$result$batch_counts$B1, 6L))
stopifnot(identical(unlist(run$result$result$covariate_columns), c("condition", "libsize")))
stopifnot(length(run$result$artifacts) == 1L)
stopifnot(identical(run$result$artifacts[[1]]$sha256,
  as.character(tools::sha256sum(file.path(out1, "corrected.tsv")))))

corrected <- read.csv(file.path(out1, "corrected.tsv"), sep = "\t", row.names = 1)
stopifnot(all(colnames(corrected) == samples))
raw_batch_gap <- abs(mean(values[, batch == "B0"]) - mean(values[, batch == "B1"]))
corrected_batch_gap <- abs(mean(as.matrix(corrected[, batch == "B0"])) - mean(as.matrix(corrected[, batch == "B1"])))
stopifnot(corrected_batch_gap < 0.2, raw_batch_gap > corrected_batch_gap * 3)
group_gap_before <- rowMeans(values[, condition == "case"]) - rowMeans(values[, condition == "control"])
group_gap_after <- rowMeans(as.matrix(corrected[, condition == "case"])) - rowMeans(as.matrix(corrected[, condition == "control"]))
stopifnot(median(abs(group_gap_after - group_gap_before)) < 0.25)

# --- determinism: same inputs, byte-identical output ---
out2 <- file.path(root, "out2")
dir.create(out2)
run2 <- run_ok(out2, "corrected.tsv")
stopifnot(identical(
  as.character(tools::sha256sum(file.path(out1, "corrected.tsv"))),
  as.character(tools::sha256sum(file.path(out2, "corrected.tsv")))
))

# --- error: batch with a single sample ---
single_path <- file.path(root, "samples-single.csv")
writeLines(c("sample,batch,condition,libsize",
  paste0(samples, ",", c(rep("B0", 11), "B1"), ",", condition, ",", libsize)), single_path)
out3 <- file.path(root, "out3")
dir.create(out3)
request_path <- make_request(out3, "corrected.tsv")
request <- fromJSON(request_path, simplifyVector = FALSE)
request$inputs[[2]]$files[[1]]$path <- single_path
request_path <- file.path(root, "request-single.json")
write(toJSON(request, auto_unbox = TRUE, digits = NA, null = "null"), request_path)
result_path <- file.path(root, "result-single.json")
code <- main(list("--request", request_path, "--result", result_path))
stopifnot(identical(code, 2L))
stopifnot(identical(fromJSON(result_path, simplifyVector = FALSE)$status, "error"))

# --- error: missing value in the matrix ---
missing_matrix <- file.path(root, "matrix-missing.csv")
lines_missing <- lines
lines_missing[3] <- sub("[0-9.]+$", "NA", lines_missing[3])
writeLines(lines_missing, missing_matrix)
out4 <- file.path(root, "out4")
dir.create(out4)
request <- fromJSON(make_request(out4, "corrected.tsv"), simplifyVector = FALSE)
request$inputs[[1]]$files[[1]]$path <- missing_matrix
request_path <- file.path(root, "request-missing.json")
write(toJSON(request, auto_unbox = TRUE, digits = NA, null = "null"), request_path)
code <- main(list("--request", request_path, "--result", file.path(root, "result-missing.json")))
stopifnot(identical(code, 2L))

# --- error: sample ids do not match ---
mismatch_path <- file.path(root, "samples-mismatch.csv")
writeLines(c("sample,batch,condition,libsize",
  paste0(c(samples[-1], "S99"), ",", batch, ",", condition, ",", libsize)), mismatch_path)
out5 <- file.path(root, "out5")
dir.create(out5)
request <- fromJSON(make_request(out5, "corrected.tsv"), simplifyVector = FALSE)
request$inputs[[2]]$files[[1]]$path <- mismatch_path
request_path <- file.path(root, "request-mismatch.json")
write(toJSON(request, auto_unbox = TRUE, digits = NA, null = "null"), request_path)
code <- main(list("--request", request_path, "--result", file.path(root, "result-mismatch.json")))
stopifnot(identical(code, 2L))

cat("combat-r validation: all checks passed\n")
