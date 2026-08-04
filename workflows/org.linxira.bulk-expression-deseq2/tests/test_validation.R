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

stopifnot(version_satisfies("4.6.1", ">=4.6.1,<4.7.0"))
stopifnot(version_satisfies("4.6.9", ">=4.6.1,<4.7.0"))
stopifnot(!version_satisfies("4.6.0", ">=4.6.1,<4.7.0"))
stopifnot(!version_satisfies("4.7.0", ">=4.6.1,<4.7.0"))
stopifnot(version_satisfies("1.52.1", ">=1.52.0,<1.53.0"))

project_library <- file.path(root, "r-library")
dir.create(project_library)
previous_library_setting <- Sys.getenv(
  "LINXIRA_BIO_WORKFLOW_R_LIBRARY", unset = NA_character_
)
previous_library_paths <- .libPaths()
on.exit({
  .libPaths(previous_library_paths)
  if (is.na(previous_library_setting)) {
    Sys.unsetenv("LINXIRA_BIO_WORKFLOW_R_LIBRARY")
  } else {
    Sys.setenv(LINXIRA_BIO_WORKFLOW_R_LIBRARY = previous_library_setting)
  }
}, add = TRUE)
Sys.setenv(LINXIRA_BIO_WORKFLOW_R_LIBRARY = project_library)
activated_library <- configure_project_library()
stopifnot(same_path(activated_library, normalizePath(
  project_library, winslash = "/", mustWork = TRUE
)))
stopifnot(same_path(.libPaths()[[1L]], activated_library))
stopifnot(validate_loaded_namespace_origins(activated_library))

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
  schema_version = "2", job_id = "validation-test", capability = PRIMARY_CAPABILITY,
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
stopifnot(identical(config$capability, PRIMARY_CAPABILITY))
for (capability in SUPPORTED_CAPABILITIES) {
  capability_request <- request
  capability_request$capability <- capability
  capability_config <- validate_request(
    capability_request, file.path(root, "output", "result.json")
  )
  stopifnot(identical(capability_config$capability, capability))
}
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
medical_error <- minimal_error_json(
  "medical-test", MEDICAL_CAPABILITY, "expected failure", "2026-01-01T00:00:00Z"
)
stopifnot(grepl('"capability":"medical.bulk-rnaseq.v1"', medical_error, fixed = TRUE))
stopifnot(grepl('"code":"research_use_only"', medical_error, fixed = TRUE))
stopifnot(grepl("no diagnosis or clinical interpretation", medical_error, fixed = TRUE))
cat("DESeq2 workflow validation tests passed\n")
