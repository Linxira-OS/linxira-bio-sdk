test_file <- sub(
  "^--file=", "",
  commandArgs(trailingOnly = FALSE)[grep("^--file=", commandArgs(trailingOnly = FALSE))]
)
script <- normalizePath(
  file.path(dirname(test_file), "..", "src", "run_deseq2.R"),
  winslash = "/", mustWork = TRUE
)
source(script, local = TRUE)

root <- tempfile("linxira-deseq2-validation-")
dir.create(root)
on.exit(unlink(root, recursive = TRUE, force = TRUE), add = TRUE)
counts_path <- file.path(root, "counts.tsv")
samples_path <- file.path(root, "samples.tsv")
writeLines(c("gene\ts1\ts2\ts3\ts4", "g1\t10\t12\t25\t30"), counts_path)
writeLines(c("sample\tcondition", "s1\tcontrol", "s2\tcontrol", "s3\ttreated", "s4\ttreated"), samples_path)
artifact <- function(id, role, path) list(
  artifact_id = id, role = role, cardinality = "single",
  files = list(list(
    file_id = id, path = path, format = "tsv", compression = "none",
    size_bytes = unname(file.info(path)$size)
  ))
)
request <- list(
  schema_version = "2", job_id = "validation-test", capability = CAPABILITY,
  inputs = list(artifact("counts", "counts", counts_path),
                artifact("samples", "sample_metadata", samples_path)),
  execution = list(mode = "local-cpu"),
  parameters = list(
    output_directory = file.path(root, "output"), feature_id_column = "gene",
    sample_id_column = "sample", condition_column = "condition",
    reference_level = "control", contrast_level = "treated"
  )
)
config <- validate_request(request, file.path(root, "output", "result.json"))
stopifnot(config$alpha == 0.05, config$min_total_count == 10L)
loaded <- load_analysis_inputs(config)
stopifnot(nrow(loaded$counts) == 1L, ncol(loaded$counts) == 4L)
stopifnot(identical(colnames(loaded$metadata), ".linxira_condition"))

malicious_column <- "stop(TRUE)"
malicious_samples <- file.path(root, "malicious-samples.tsv")
writeLines(c(
  paste("sample", malicious_column, sep = "\t"),
  "s1\tcontrol", "s2\tcontrol", "s3\ttreated", "s4\ttreated"
), malicious_samples)
malicious_request <- request
malicious_request$inputs[[2L]] <- artifact(
  "samples", "sample_metadata", malicious_samples
)
malicious_request$parameters$condition_column <- malicious_column
malicious_config <- validate_request(
  malicious_request, file.path(root, "output", "result.json")
)
malicious_loaded <- load_analysis_inputs(malicious_config)
stopifnot(identical(colnames(malicious_loaded$metadata), ".linxira_condition"))

bad <- request
bad$unexpected <- TRUE
rejected <- tryCatch({ validate_request(bad, file.path(root, "output", "result.json")); FALSE },
                     request_error = function(error) TRUE)
stopifnot(rejected)

error_target <- file.path(root, "failed-output", "result.json")
stopifnot(write_error_json_atomic(error_target, '{"status":"error"}'))
stopifnot(identical(readLines(error_target, warn = FALSE), '{"status":"error"}'))
cat("DESeq2 workflow validation tests passed\n")
