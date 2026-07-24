#!/usr/bin/env Rscript

PACK_ID <- "org.linxira.bulk-expression-deseq2"
PACK_VERSION <- "0.1.0"
CAPABILITY <- "expression.deseq2.v1"
EXPECTED_R <- "4.4.3"
EXPECTED_PACKAGES <- c(DESeq2 = "1.46.0", jsonlite = "1.8.9", digest = "0.6.37")

request_error <- function(message) {
  stop(structure(list(message = message, call = NULL), class = c("request_error", "error", "condition")))
}

require_object <- function(value, context) {
  if (!is.list(value) || is.null(names(value))) {
    request_error(sprintf("%s must be an object", context))
  }
  value
}

require_exact_keys <- function(value, required, optional = character(), context) {
  missing <- setdiff(required, names(value))
  unknown <- setdiff(names(value), c(required, optional))
  if (length(missing) > 0L) {
    request_error(sprintf("%s is missing: %s", context, paste(sort(missing), collapse = ", ")))
  }
  if (length(unknown) > 0L) {
    request_error(sprintf(
      "%s has unsupported fields: %s", context, paste(sort(unknown), collapse = ", ")
    ))
  }
}

require_string <- function(value, context) {
  if (!is.character(value) || length(value) != 1L || is.na(value) || !nzchar(value)) {
    request_error(sprintf("%s must be a non-empty string", context))
  }
  value
}

require_number <- function(value, context, minimum = -Inf, maximum = Inf) {
  if (!is.numeric(value) || length(value) != 1L || is.na(value) || !is.finite(value) ||
      value < minimum || value > maximum) {
    request_error(sprintf("%s must be a finite number in [%s, %s]", context, minimum, maximum))
  }
  as.numeric(value)
}

require_nonnegative_integer <- function(value, context) {
  number <- require_number(value, context, 0, .Machine$integer.max)
  if (number != floor(number)) {
    request_error(sprintf("%s must be an integer", context))
  }
  as.integer(number)
}

require_nonnegative_whole <- function(value, context) {
  number <- require_number(value, context, 0, 2^53)
  if (number != floor(number)) {
    request_error(sprintf("%s must be an integer", context))
  }
  number
}

is_array <- function(value) {
  is.list(value) && is.null(names(value))
}

canonical_existing <- function(path) {
  normalizePath(path, winslash = "/", mustWork = TRUE)
}

canonical_target <- function(path) {
  normalizePath(path, winslash = "/", mustWork = FALSE)
}

same_path <- function(left, right) {
  if (.Platform$OS.type == "windows") {
    identical(tolower(left), tolower(right))
  } else {
    identical(left, right)
  }
}

validate_file_artifact <- function(artifact, expected_role, context) {
  artifact <- require_object(artifact, context)
  require_exact_keys(
    artifact, c("artifact_id", "role", "cardinality", "files"), character(), context
  )
  require_string(artifact$artifact_id, sprintf("%s.artifact_id", context))
  if (!identical(artifact$role, expected_role)) {
    request_error(sprintf("%s.role must be '%s'", context, expected_role))
  }
  if (!identical(artifact$cardinality, "single")) {
    request_error(sprintf("%s.cardinality must be 'single'", context))
  }
  if (!is_array(artifact$files) || length(artifact$files) != 1L) {
    request_error(sprintf("%s.files must contain exactly one file", context))
  }
  file <- require_object(artifact$files[[1L]], sprintf("%s.files[0]", context))
  require_exact_keys(
    file, c("file_id", "path", "format", "compression", "size_bytes"), "sha256",
    sprintf("%s.files[0]", context)
  )
  require_string(file$file_id, sprintf("%s.files[0].file_id", context))
  path <- require_string(file$path, sprintf("%s.files[0].path", context))
  if (!file.exists(path) || dir.exists(path)) {
    request_error(sprintf("input file does not exist: %s", path))
  }
  if (!file$format %in% c("csv", "tsv")) {
    request_error(sprintf("%s.files[0].format must be 'csv' or 'tsv'", context))
  }
  if (!identical(file$compression, "none")) {
    request_error("compressed input is not supported by this pack version")
  }
  declared_size <- require_nonnegative_whole(
    file$size_bytes, sprintf("%s.files[0].size_bytes", context)
  )
  actual_size <- file.info(path)$size
  if (declared_size != actual_size) {
    request_error(sprintf(
      "declared input size %s does not match actual size %s", declared_size, actual_size
    ))
  }
  declared_sha256 <- NULL
  if (!is.null(file$sha256)) {
    declared_sha256 <- require_string(file$sha256, sprintf("%s.files[0].sha256", context))
    if (!grepl("^[A-Fa-f0-9]{64}$", declared_sha256)) {
      request_error(sprintf("%s.files[0].sha256 must contain 64 hexadecimal characters", context))
    }
  }
  list(
    artifact_id = artifact$artifact_id,
    path = canonical_existing(path),
    format = file$format,
    declared_size = declared_size,
    declared_sha256 = declared_sha256
  )
}

validate_request <- function(document, result_path) {
  request <- require_object(document, "request")
  require_exact_keys(
    request,
    c("schema_version", "job_id", "capability", "inputs", "execution", "parameters"),
    character(), "request"
  )
  if (!identical(request$schema_version, "2")) request_error("schema_version must be '2'")
  require_string(request$job_id, "job_id")
  if (!identical(request$capability, CAPABILITY)) {
    request_error(sprintf("capability must be '%s'", CAPABILITY))
  }
  execution <- require_object(request$execution, "execution")
  require_exact_keys(execution, "mode", character(), "execution")
  if (!identical(execution$mode, "local-cpu")) {
    request_error("execution.mode must be 'local-cpu'")
  }
  if (!is_array(request$inputs) || length(request$inputs) != 2L) {
    request_error("inputs must contain counts and sample_metadata artifacts")
  }
  roles <- vapply(request$inputs, function(x) {
    if (is.list(x) && is.character(x$role) && length(x$role) == 1L) x$role else ""
  }, character(1L))
  if (!setequal(roles, c("counts", "sample_metadata")) || anyDuplicated(roles)) {
    request_error("inputs must contain one 'counts' and one 'sample_metadata' role")
  }
  counts <- validate_file_artifact(
    request$inputs[[which(roles == "counts")]], "counts", "inputs[counts]"
  )
  samples <- validate_file_artifact(
    request$inputs[[which(roles == "sample_metadata")]], "sample_metadata",
    "inputs[sample_metadata]"
  )
  if (same_path(counts$path, samples$path)) {
    request_error("counts and sample metadata must be different files")
  }

  parameters <- require_object(request$parameters, "parameters")
  require_exact_keys(
    parameters,
    c(
      "output_directory", "feature_id_column", "sample_id_column", "condition_column",
      "reference_level", "contrast_level"
    ),
    c("alpha", "min_total_count"), "parameters"
  )
  output_directory <- require_string(parameters$output_directory, "parameters.output_directory")
  if (!dir.exists(dirname(output_directory))) {
    request_error(sprintf("output parent directory does not exist: %s", dirname(output_directory)))
  }
  if (file.exists(output_directory)) {
    request_error("parameters.output_directory must not already exist")
  }
  output_directory <- canonical_target(output_directory)
  expected_result <- file.path(output_directory, "result.json")
  supplied_result <- canonical_target(result_path)
  if (!same_path(expected_result, supplied_result)) {
    request_error("--result must be <parameters.output_directory>/result.json")
  }
  feature_id_column <- require_string(parameters$feature_id_column, "parameters.feature_id_column")
  sample_id_column <- require_string(parameters$sample_id_column, "parameters.sample_id_column")
  condition_column <- require_string(parameters$condition_column, "parameters.condition_column")
  reference_level <- require_string(parameters$reference_level, "parameters.reference_level")
  contrast_level <- require_string(parameters$contrast_level, "parameters.contrast_level")
  if (identical(reference_level, contrast_level)) {
    request_error("reference_level and contrast_level must differ")
  }
  alpha <- if (is.null(parameters$alpha)) 0.05 else
    require_number(parameters$alpha, "parameters.alpha", .Machine$double.eps, 1)
  min_total_count <- if (is.null(parameters$min_total_count)) 10L else
    require_nonnegative_integer(parameters$min_total_count, "parameters.min_total_count")
  list(
    job_id = request$job_id,
    counts = counts,
    samples = samples,
    output_directory = output_directory,
    feature_id_column = feature_id_column,
    sample_id_column = sample_id_column,
    condition_column = condition_column,
    reference_level = reference_level,
    contrast_level = contrast_level,
    alpha = alpha,
    min_total_count = min_total_count
  )
}

read_character_table <- function(path, format) {
  reader <- if (identical(format, "csv")) utils::read.csv else utils::read.delim
  tryCatch(
    reader(
      path, header = TRUE, check.names = FALSE, stringsAsFactors = FALSE,
      colClasses = "character", na.strings = character(), comment.char = "",
      fileEncoding = "UTF-8"
    ),
    error = function(error) request_error(sprintf("cannot parse %s: %s", path, error$message))
  )
}

load_analysis_inputs <- function(config) {
  raw_counts <- read_character_table(config$counts$path, config$counts$format)
  if (any(!nzchar(colnames(raw_counts))) || anyDuplicated(colnames(raw_counts))) {
    request_error("count matrix column names must be non-empty and unique")
  }
  if (!config$feature_id_column %in% colnames(raw_counts)) {
    request_error(sprintf("count matrix lacks feature column '%s'", config$feature_id_column))
  }
  feature_ids <- raw_counts[[config$feature_id_column]]
  if (length(feature_ids) == 0L || any(!nzchar(feature_ids)) || anyDuplicated(feature_ids)) {
    request_error("feature identifiers must be non-empty and unique")
  }
  count_columns <- setdiff(colnames(raw_counts), config$feature_id_column)
  if (length(count_columns) < 2L) request_error("count matrix requires at least two samples")
  raw_values <- as.matrix(raw_counts[count_columns])
  if (any(!grepl("^[0-9]+$", raw_values))) {
    request_error("count matrix values must be non-negative integer literals")
  }
  numeric_values <- suppressWarnings(matrix(
    as.numeric(raw_values), nrow = nrow(raw_values), ncol = ncol(raw_values),
    dimnames = list(feature_ids, count_columns)
  ))
  if (any(!is.finite(numeric_values)) || any(numeric_values > .Machine$integer.max)) {
    request_error("count matrix contains values outside the supported 32-bit integer range")
  }
  counts <- matrix(
    as.integer(numeric_values), nrow = nrow(numeric_values), ncol = ncol(numeric_values),
    dimnames = dimnames(numeric_values)
  )

  samples <- read_character_table(config$samples$path, config$samples$format)
  if (any(!nzchar(colnames(samples))) || anyDuplicated(colnames(samples))) {
    request_error("sample metadata column names must be non-empty and unique")
  }
  needed <- c(config$sample_id_column, config$condition_column)
  missing <- setdiff(needed, colnames(samples))
  if (length(missing) > 0L) {
    request_error(sprintf("sample metadata lacks columns: %s", paste(missing, collapse = ", ")))
  }
  sample_ids <- samples[[config$sample_id_column]]
  conditions <- samples[[config$condition_column]]
  if (any(!nzchar(sample_ids)) || anyDuplicated(sample_ids)) {
    request_error("sample identifiers must be non-empty and unique")
  }
  if (any(!nzchar(conditions))) request_error("condition values must be non-empty")
  if (!setequal(sample_ids, colnames(counts))) {
    request_error("count matrix columns and sample metadata identifiers must match exactly")
  }
  if (!all(c(config$reference_level, config$contrast_level) %in% conditions)) {
    request_error("reference_level and contrast_level must both occur in sample metadata")
  }
  if (!setequal(unique(conditions), c(config$reference_level, config$contrast_level))) {
    request_error("sample metadata must contain exactly the reference and contrast levels")
  }
  level_counts <- table(conditions)
  if (any(level_counts < 2L)) {
    request_error("each condition requires at least two biological samples")
  }
  order <- match(colnames(counts), sample_ids)
  metadata <- data.frame(
    .linxira_condition = conditions[order], row.names = sample_ids[order], check.names = FALSE
  )
  metadata$.linxira_condition <- stats::relevel(
    factor(metadata$.linxira_condition), ref = config$reference_level
  )
  list(counts = counts, metadata = metadata)
}

check_runtime <- function() {
  actual_r <- paste(R.version$major, R.version$minor, sep = ".")
  if (!identical(actual_r, EXPECTED_R)) {
    stop(sprintf("locked runtime requires R %s, found R %s", EXPECTED_R, actual_r))
  }
  for (package in names(EXPECTED_PACKAGES)) {
    if (!requireNamespace(package, quietly = TRUE)) {
      stop(sprintf("locked dependency %s %s is not installed", package, EXPECTED_PACKAGES[[package]]))
    }
    actual <- as.character(utils::packageVersion(package))
    if (!identical(actual, EXPECTED_PACKAGES[[package]])) {
      stop(sprintf(
        "locked dependency requires %s %s, found %s", package, EXPECTED_PACKAGES[[package]], actual
      ))
    }
  }
}

artifact_record <- function(id, role, path, final_path) {
  list(
    artifact_id = id,
    role = role,
    kind = "table",
    path = final_path,
    format = "csv",
    media_type = "text/csv",
    size_bytes = unname(file.info(path)$size),
    sha256 = digest::digest(file = path, algo = "sha256", serialize = FALSE)
  )
}

run_analysis <- function(config, started_at) {
  if (file.info(config$counts$path)$size != config$counts$declared_size ||
      file.info(config$samples$path)$size != config$samples$declared_size) {
    request_error("an input size changed after request validation")
  }
  counts_sha <- digest::digest(file = config$counts$path, algo = "sha256", serialize = FALSE)
  samples_sha <- digest::digest(file = config$samples$path, algo = "sha256", serialize = FALSE)
  if (!is.null(config$counts$declared_sha256) &&
      !identical(tolower(config$counts$declared_sha256), counts_sha)) {
    request_error("declared counts SHA-256 does not match file content")
  }
  if (!is.null(config$samples$declared_sha256) &&
      !identical(tolower(config$samples$declared_sha256), samples_sha)) {
    request_error("declared sample metadata SHA-256 does not match file content")
  }
  input <- load_analysis_inputs(config)
  keep <- rowSums(input$counts) >= config$min_total_count
  if (!any(keep)) request_error("no features remain after min_total_count filtering")
  filtered <- input$counts[keep, , drop = FALSE]
  design <- ~ .linxira_condition
  dataset <- DESeq2::DESeqDataSetFromMatrix(
    countData = filtered, colData = input$metadata, design = design
  )
  dataset <- DESeq2::DESeq(dataset, quiet = TRUE)
  results <- DESeq2::results(
    dataset,
    contrast = c(".linxira_condition", config$contrast_level, config$reference_level),
    alpha = config$alpha
  )
  differential <- data.frame(
    feature_id = rownames(results),
    base_mean = results$baseMean,
    log2_fold_change = results$log2FoldChange,
    standard_error = results$lfcSE,
    statistic = results$stat,
    p_value = results$pvalue,
    adjusted_p_value = results$padj,
    stringsAsFactors = FALSE
  )
  differential <- differential[order(differential$adjusted_p_value, na.last = TRUE), , drop = FALSE]
  normalized_matrix <- DESeq2::counts(dataset, normalized = TRUE)
  normalized <- data.frame(feature_id = rownames(normalized_matrix), normalized_matrix,
                           check.names = FALSE, stringsAsFactors = FALSE)

  output_parent <- dirname(config$output_directory)
  staging <- tempfile(pattern = ".linxira-deseq2-", tmpdir = output_parent)
  if (!dir.create(staging, mode = "0700")) stop("cannot create staging directory")
  committed <- FALSE
  on.exit(if (!committed && dir.exists(staging)) unlink(staging, recursive = TRUE, force = TRUE), add = TRUE)
  differential_path <- file.path(staging, "differential-expression.csv")
  normalized_path <- file.path(staging, "normalized-counts.csv")
  utils::write.csv(
    differential, differential_path, row.names = FALSE, na = "", fileEncoding = "UTF-8"
  )
  utils::write.csv(
    normalized, normalized_path, row.names = FALSE, na = "", fileEncoding = "UTF-8"
  )

  if (!identical(
    counts_sha,
    digest::digest(file = config$counts$path, algo = "sha256", serialize = FALSE)
  ) || !identical(
    samples_sha,
    digest::digest(file = config$samples$path, algo = "sha256", serialize = FALSE)
  )) {
    request_error("an input file changed while the workflow was running")
  }
  pack_root <- normalizePath(file.path(SCRIPT_DIRECTORY, ".."), winslash = "/", mustWork = TRUE)
  lock_path <- file.path(pack_root, "dependencies.lock.json")
  final_differential <- file.path(config$output_directory, "differential-expression.csv")
  final_normalized <- file.path(config$output_directory, "normalized-counts.csv")
  significant <- sum(!is.na(differential$adjusted_p_value) &
                       differential$adjusted_p_value <= config$alpha)
  loaded_software <- lapply(sort(loadedNamespaces()), function(package) list(
    name = package,
    version = as.character(utils::packageVersion(package)),
    package_id = package
  ))
  result <- list(
    schema_version = "2",
    job_id = config$job_id,
    capability = CAPABILITY,
    status = "ok",
    result = list(
      input_features = nrow(input$counts),
      analyzed_features = nrow(filtered),
      filtered_features = nrow(input$counts) - nrow(filtered),
      samples = ncol(filtered),
      significant_features = significant,
      alpha = config$alpha,
      min_total_count = config$min_total_count,
      contrast = list(level = config$contrast_level, reference = config$reference_level),
      effective_parameters = list(
        feature_id_column = config$feature_id_column,
        sample_id_column = config$sample_id_column,
        condition_column = config$condition_column,
        reference_level = config$reference_level,
        contrast_level = config$contrast_level,
        alpha = config$alpha,
        min_total_count = config$min_total_count
      )
    ),
    artifacts = list(
      artifact_record("differential-expression", "differential-expression", differential_path,
                      final_differential),
      artifact_record("normalized-counts", "normalized-counts", normalized_path, final_normalized)
    ),
    provenance = list(
      engine_version = PACK_VERSION,
      execution_mode = "local-cpu",
      started_at = started_at,
      finished_at = format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC"),
      software = c(list(list(name = "R", version = EXPECTED_R)), loaded_software),
      input_sha256 = list(counts = counts_sha, sample_metadata = samples_sha),
      command = c("Rscript", "src/run_deseq2.R", "--request", "<request>", "--result", "<result>"),
      dependency_lock_sha256 = digest::digest(
        file = lock_path, algo = "sha256", serialize = FALSE
      )
    ),
    diagnostics = list()
  )
  jsonlite::write_json(
    result, file.path(staging, "result.json"), auto_unbox = TRUE, pretty = TRUE,
    na = "null", null = "null", digits = NA
  )
  if (!file.rename(staging, config$output_directory)) {
    stop("atomic output directory activation failed")
  }
  committed <- TRUE
  result
}

parse_arguments <- function(arguments) {
  if (length(arguments) != 4L) stop("usage: run_deseq2.R --request REQUEST --result RESULT")
  request_index <- which(arguments == "--request")
  result_index <- which(arguments == "--result")
  if (length(request_index) != 1L || length(result_index) != 1L ||
      request_index == length(arguments) || result_index == length(arguments)) {
    stop("usage: run_deseq2.R --request REQUEST --result RESULT")
  }
  values <- c(request_index + 1L, result_index + 1L)
  flags <- c(request_index, result_index)
  if (!setequal(c(flags, values), seq_along(arguments))) {
    stop("unsupported or duplicate command-line arguments")
  }
  list(request = arguments[[request_index + 1L]], result = arguments[[result_index + 1L]])
}

minimal_error_json <- function(job_id, message, started_at) {
  sprintf(
    paste0(
      '{"schema_version":"2","job_id":%s,"capability":"%s","status":"error",',
      '"result":{},"artifacts":[],"provenance":{"engine_version":"%s",',
      '"execution_mode":"local-cpu","started_at":"%s","finished_at":"%s"},',
      '"diagnostics":[{"code":"workflow_failed","severity":"error","message":%s}]}'
    ),
    encodeString(job_id, quote = '"'), CAPABILITY, PACK_VERSION, started_at,
    format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC"), encodeString(message, quote = '"')
  )
}

write_error_json_atomic <- function(result_path, payload) {
  target <- normalizePath(result_path, winslash = "/", mustWork = FALSE)
  parent <- dirname(target)
  if (dir.exists(parent)) {
    if (file.exists(target) || dir.exists(target)) return(FALSE)
    temporary <- tempfile(pattern = ".linxira-error-", tmpdir = parent)
    writeLines(enc2utf8(payload), temporary, useBytes = TRUE)
    if (!file.rename(temporary, target)) {
      unlink(temporary, force = TRUE)
      return(FALSE)
    }
    return(TRUE)
  }
  grandparent <- dirname(parent)
  if (!dir.exists(grandparent) || file.exists(parent)) return(FALSE)
  staging <- tempfile(pattern = ".linxira-error-", tmpdir = grandparent)
  if (!dir.create(staging, mode = "0700")) return(FALSE)
  committed <- FALSE
  on.exit(if (!committed && dir.exists(staging)) unlink(staging, recursive = TRUE, force = TRUE), add = TRUE)
  writeLines(enc2utf8(payload), file.path(staging, basename(target)), useBytes = TRUE)
  if (!file.rename(staging, parent)) return(FALSE)
  committed <- TRUE
  TRUE
}

main <- function() {
  started_at <- format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC")
  job_id <- "workflow-error"
  result_path <- NULL
  status <- tryCatch({
    options <- parse_arguments(commandArgs(trailingOnly = TRUE))
    result_path <- options$result
    if (!file.exists(options$request) || dir.exists(options$request)) {
      request_error(sprintf("request file does not exist: %s", options$request))
    }
    if (!requireNamespace("jsonlite", quietly = TRUE)) {
      stop("locked dependency jsonlite 1.8.9 is not installed")
    }
    document <- jsonlite::fromJSON(options$request, simplifyVector = FALSE)
    if (is.list(document) && is.character(document$job_id) && length(document$job_id) == 1L &&
        !is.na(document$job_id) && nzchar(document$job_id)) {
      job_id <- document$job_id
    }
    config <- validate_request(document, options$result)
    check_runtime()
    result <- run_analysis(config, started_at)
    cat(jsonlite::toJSON(result, auto_unbox = TRUE, na = "null", null = "null", digits = NA), "\n")
    0L
  }, error = function(error) {
    payload <- minimal_error_json(job_id, conditionMessage(error), started_at)
    if (!is.null(result_path)) {
      try(write_error_json_atomic(result_path, payload), silent = TRUE)
    }
    cat(payload, "\n")
    message(sprintf("%s: %s", PACK_ID, conditionMessage(error)))
    2L
  })
  status
}

arguments_all <- commandArgs(trailingOnly = FALSE)
file_argument <- grep("^--file=", arguments_all, value = TRUE)
SCRIPT_DIRECTORY <- if (length(file_argument) == 1L) {
  dirname(normalizePath(sub("^--file=", "", file_argument), winslash = "/", mustWork = TRUE))
} else {
  getwd()
}

if (sys.nframe() == 0L) quit(status = main(), save = "no")
