test_file <- sub(
  "^--file=", "",
  commandArgs(trailingOnly = FALSE)[grep("^--file=", commandArgs(trailingOnly = FALSE))]
)
script <- normalizePath(
  file.path(dirname(test_file), "..", "src", "run_wgcna.R"),
  winslash = "/", mustWork = TRUE
)
source(script, local = TRUE)

root <- tempfile("linxira-wgcna-validation-")
dir.create(root)
on.exit(unlink(root, recursive = TRUE, force = TRUE), add = TRUE)

stopifnot(version_satisfies("4.6.1", ">=4.6.1,<4.7.0"))
stopifnot(version_satisfies("4.6.9", ">=4.6.1,<4.7.0"))
stopifnot(!version_satisfies("4.6.0", ">=4.6.1,<4.7.0"))
stopifnot(!version_satisfies("4.7.0", ">=4.6.1,<4.7.0"))
stopifnot(version_satisfies("1.74.0", ">=1.72,<2.0"))

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

expression_path <- file.path(root, "expression.tsv")
rows <- paste0("g", seq_len(20))
samples <- paste0("s", seq_len(6))
matrix_lines <- c(
  paste(c("gene", samples), collapse = "\t"),
  vapply(rows, function(gene) {
    paste(c(gene, as.character(sample(seq_len(50), length(samples), replace = TRUE))), collapse = "\t")
  }, character(1L))
)
writeLines(matrix_lines, expression_path)

artifact <- function(id, role, path) list(
  artifact_id = id, role = role, cardinality = "single",
  files = list(list(
    file_id = id, path = path, format = "tsv", compression = "none",
    size_bytes = unname(file.info(path)$size)
  ))
)
request <- list(
  schema_version = "2", job_id = "validation-test", capability = PRIMARY_CAPABILITY,
  inputs = list(artifact("expression", "expression", expression_path)),
  execution = list(mode = "local-cpu"),
  parameters = list(output_directory = file.path(root, "output"))
)
config <- validate_request(request, file.path(root, "output", "result.json"))
stopifnot(config$min_expression == 1, config$min_samples == 3L)
stopifnot(config$min_module_size == 30L, config$merge_cut_height == 0.25)
stopifnot(identical(config$network_type, "signed"))
stopifnot(config$power == 0, isTRUE(config$log_transform), config$threads == 1L)
stopifnot(identical(config$capability, PRIMARY_CAPABILITY))

custom <- request
custom$parameters <- list(
  output_directory = file.path(root, "custom-output"),
  min_expression = 2, min_samples = 2L, min_module_size = 10L,
  merge_cut_height = 0.5, network_type = "unsigned", power = 6L,
  log_transform = FALSE, threads = 2L
)
custom_config <- validate_request(
  custom, file.path(root, "custom-output", "result.json")
)
stopifnot(custom_config$min_expression == 2, custom_config$min_samples == 2L)
stopifnot(custom_config$min_module_size == 10L, custom_config$merge_cut_height == 0.5)
stopifnot(identical(custom_config$network_type, "unsigned"))
stopifnot(custom_config$power == 6L, !isTRUE(custom_config$log_transform), custom_config$threads == 2L)

bad_network <- request
bad_network$parameters$network_type <- "hard"
rejected <- tryCatch({
  validate_request(bad_network, file.path(root, "output", "result.json")); FALSE
}, request_error = function(error) TRUE)
stopifnot(rejected)

bad <- request
bad$unexpected <- TRUE
rejected <- tryCatch({
  validate_request(bad, file.path(root, "output", "result.json")); FALSE
}, request_error = function(error) TRUE)
stopifnot(rejected)

error_target <- file.path(root, "failed-output", "result.json")
stopifnot(write_error_json_atomic(error_target, '{"status":"error"}'))
stopifnot(identical(readLines(error_target, warn = FALSE), '{"status":"error"}'))
cat("WGCNA workflow validation tests passed\n")
