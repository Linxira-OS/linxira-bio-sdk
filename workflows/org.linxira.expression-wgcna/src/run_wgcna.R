#!/usr/bin/env Rscript

PACK_ID <- "org.linxira.expression-wgcna"
PACK_VERSION <- "0.1.0"
PRIMARY_CAPABILITY <- "expression.wgcna.v1"
SUPPORTED_CAPABILITIES <- c(PRIMARY_CAPABILITY)
PREFERRED_R <- "4.4.0"
R_VERSION_REQUIREMENT <- ">=4.3.0,<4.6.0"
PACKAGE_REQUIREMENTS <- c(
  WGCNA = ">=1.72,<2.0",
  jsonlite = ">=1.8.9,<3.0.0",
  digest = ">=0.6.37,<0.7.0"
)

request_error <- function(message) {
  stop(structure(list(message = message, call = NULL), class = c("request_error", "error", "condition")))
}

require_object <- function(value, context) {
  if (!is.list(value) || is.null(names(value))) {
    request_error(sprintf("%s must be an object", context))
  }
  value
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

canonical_existing <- function(path) {
  normalizePath(path, winslash = "/", mustWork = TRUE)
}

same_path <- function(left, right) {
  if (.Platform$OS.type == "windows") {
    identical(tolower(left), tolower(right))
  } else {
    identical(left, right)
  }
}

version_satisfies <- function(actual, requirement) {
  actual_version <- tryCatch(numeric_version(actual), error = function(error) NULL)
  if (is.null(actual_version)) return(FALSE)
  clauses <- trimws(strsplit(requirement, ",", fixed = TRUE)[[1L]])
  if (length(clauses) == 0L || any(!nzchar(clauses))) return(FALSE)
  all(vapply(clauses, function(clause) {
    match <- regexec("^(>=|<=|==|>|<)([0-9]+(?:\\.[0-9]+)*)$", clause, perl = TRUE)
    parts <- regmatches(clause, match)[[1L]]
    if (length(parts) != 3L) return(FALSE)
    expected <- tryCatch(numeric_version(parts[[3L]]), error = function(error) NULL)
    if (is.null(expected)) return(FALSE)
    switch(
      parts[[2L]],
      ">=" = actual_version >= expected,
      "<=" = actual_version <= expected,
      "==" = actual_version == expected,
      ">" = actual_version > expected,
      "<" = actual_version < expected,
      FALSE
    )
  }, logical(1L)))
}

path_is_within <- function(path, root) {
  path <- normalizePath(path, winslash = "/", mustWork = TRUE)
  root <- normalizePath(root, winslash = "/", mustWork = TRUE)
  if (.Platform$OS.type == "windows") {
    path <- tolower(path)
    root <- tolower(root)
  }
  identical(path, root) || startsWith(path, paste0(root, "/"))
}

configure_project_library <- function() {
  configured <- Sys.getenv("LINXIRA_BIO_WORKFLOW_R_LIBRARY", unset = "")
  if (!nzchar(configured)) {
    stop("LINXIRA_BIO_WORKFLOW_R_LIBRARY must name the existing project-isolated R package library")
  }
  if (!dir.exists(configured)) {
    stop(sprintf("project-isolated R package library does not exist: %s", configured))
  }
  library <- normalizePath(configured, winslash = "/", mustWork = TRUE)
  if (path_is_within(library, R.home())) {
    stop("project-isolated R package library must be outside R_HOME")
  }
  Sys.setenv(R_LIBS_USER = library)
  .libPaths(c(library, .Library))
  active <- normalizePath(.libPaths()[[1L]], winslash = "/", mustWork = TRUE)
  if (!same_path(active, library)) {
    stop("project-isolated R package library could not be activated")
  }
  library
}

package_from_project_library <- function(package, library) {
  location <- tryCatch(
    find.package(package, lib.loc = .libPaths(), quiet = TRUE),
    error = function(error) ""
  )
  nzchar(location) && path_is_within(location, library)
}

check_runtime <- function(project_library) {
  actual_r <- paste(R.version$major, R.version$minor, sep = ".")
  if (!version_satisfies(actual_r, R_VERSION_REQUIREMENT)) {
    stop(sprintf(
      "workflow requires R %s (preferred %s), found R %s",
      R_VERSION_REQUIREMENT, PREFERRED_R, actual_r
    ))
  }
  for (package in names(PACKAGE_REQUIREMENTS)) {
    if (!requireNamespace(package, quietly = TRUE)) {
      stop(sprintf(
        "dependency %s %s is not installed in the project library",
        package, PACKAGE_REQUIREMENTS[[package]]
      ))
    }
    if (!package_from_project_library(package, project_library)) {
      stop(sprintf("dependency %s did not resolve from the project library", package))
    }
    actual <- as.character(utils::packageVersion(package))
    if (!version_satisfies(actual, PACKAGE_REQUIREMENTS[[package]])) {
      stop(sprintf(
        "dependency requires %s %s, found %s",
        package, PACKAGE_REQUIREMENTS[[package]], actual
      ))
    }
  }
  invisible(list(r = actual_r, library = project_library))
}

read_expression_matrix <- function(path) {
  first <- readLines(path, n = 1L, warn = FALSE)
  sep <- if (grepl("\t", first)) "\t" else ","
  tryCatch(
    utils::read.table(
      path, header = TRUE, sep = sep, row.names = 1L, check.names = FALSE,
      stringsAsFactors = FALSE, comment.char = "", fileEncoding = "UTF-8"
    ),
    error = function(error) request_error(sprintf("cannot parse expression matrix: %s", error$message))
  )
}

minimal_error_json <- function(job_id, capability, message, started_at) {
  sprintf(
    '{"schema_version":"2","job_id":%s,"capability":%s,"status":"error","result":{},' %+%
    '"artifacts":[],"provenance":{"engine_version":"%s","execution_mode":"local-cpu",' %+%
    '"started_at":"%s","finished_at":"%s"},"diagnostics":[{"code":"workflow_failed",' %+%
    '"severity":"error","message":%s}]}',
    encodeString(job_id, quote = '"'), encodeString(capability, quote = '"'),
    PACK_VERSION, started_at, format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC"),
    encodeString(message, quote = '"')
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

run_analysis <- function(config, started_at, project_library) {
  expr <- read_expression_matrix(config$expression_path)
  if (nrow(expr) < 10L) request_error("expression matrix requires at least 10 genes")
  if (ncol(expr) < 5L) request_error("expression matrix requires at least 5 samples")

  expr <- as.matrix(expr)
  if (any(!is.finite(expr)) || any(expr < 0)) {
    request_error("expression matrix values must be non-negative and finite")
  }

  good_genes <- apply(expr, 1L, function(row) {
    sum(row > config$min_expression) >= config$min_samples
  })
  if (!any(good_genes)) request_error("no genes pass the minimum expression filter")
  filtered <- expr[good_genes, , drop = FALSE]

  if (config$log_transform) {
    filtered <- log2(filtered + 1)
  }

  WGCNA::allowWGCNAThreads(nThreads = config$threads)
  WGCNA::enableWGCNAThreads(nThreads = config$threads)

  powers <- if (config$power == 0) {
    c(seq(1, 10, by = 1), seq(12, 30, by = 2))
  } else {
    config$power
  }

  sft <- WGCNA::pickSoftThreshold(
    t(filtered), powerVector = powers, networkType = config$network_type,
    verbose = 0L, moreNetworkConcepts = TRUE
  )

  if (config$power == 0) {
    fit_indices <- sft$fitIndices
    best_row <- fit_indices[which.max(fit_indices$SFT.R.sq), ]
    best_power <- best_row$Power
    if (best_power < 1) best_power <- 6
  } else {
    best_power <- config$power
  }

  net <- WGCNA::blockwiseModules(
    t(filtered), power = best_power, networkType = config$network_type,
    minModuleSize = config$min_module_size, mergeCutHeight = config$merge_cut_height,
    numericLabels = TRUE, pamRespectsDendro = FALSE, verbose = 0L,
    maxBlockSize = ncol(filtered) + 1L
  )

  module_labels <- net$colors
  module_counts <- table(module_labels)
  module_eigengenes <- net$MEs

  output_parent <- dirname(config$output_directory)
  staging <- tempfile(pattern = ".linxira-wgcna-", tmpdir = output_parent)
  if (!dir.create(staging, mode = "0700")) stop("cannot create staging directory")
  committed <- FALSE
  on.exit(if (!committed && dir.exists(staging)) unlink(staging, recursive = TRUE, force = TRUE), add = TRUE)

  modules_path <- file.path(staging, "module-assignments.csv")
  module_df <- data.frame(
    gene = rownames(filtered),
    module = module_labels,
    stringsAsFactors = FALSE
  )
  utils::write.csv(module_df, modules_path, row.names = FALSE, na = "", fileEncoding = "UTF-8")

  eigengenes_path <- file.path(staging, "module-eigengenes.csv")
  eigengene_df <- data.frame(
    sample = colnames(filtered),
    module_eigengenes,
    check.names = FALSE,
    stringsAsFactors = FALSE
  )
  utils::write.csv(eigengene_df, eigengenes_path, row.names = FALSE, na = "", fileEncoding = "UTF-8")

  summary_path <- file.path(staging, "module-summary.csv")
  summary_df <- data.frame(
    module = as.integer(names(module_counts)),
    gene_count = as.integer(module_counts),
    stringsAsFactors = FALSE
  )
  utils::write.csv(summary_df, summary_path, row.names = FALSE, na = "", fileEncoding = "UTF-8")

  scale_free_path <- file.path(staging, "scale-free-fit.csv")
  sft_df <- data.frame(
    Power = sft$fitIndices[, 1L],
    SFT_R_sq = signif(sft$fitIndices[, 2L], 4L),
    mean_connectivity = signif(sft$fitIndices[, 5L], 4L),
    stringsAsFactors = FALSE
  )
  utils::write.csv(sft_df, scale_free_path, row.names = FALSE, na = "", fileEncoding = "UTF-8")

  pack_root <- normalizePath(file.path(SCRIPT_DIRECTORY, ".."), winslash = "/", mustWork = TRUE)
  lock_path <- file.path(pack_root, "dependencies.lock.json")

  loaded_software <- lapply(sort(loadedNamespaces()), function(package) list(
    name = package,
    version = as.character(utils::packageVersion(package)),
    package_id = package
  ))

  result <- list(
    schema_version = "2",
    job_id = config$job_id,
    capability = config$capability,
    status = "ok",
    result = list(
      input_genes = nrow(expr),
      analyzed_genes = nrow(filtered),
      filtered_genes = nrow(expr) - nrow(filtered),
      samples = ncol(filtered),
      total_modules = length(unique(module_labels)),
      best_power = best_power,
      min_module_size = config$min_module_size,
      merge_cut_height = config$merge_cut_height,
      network_type = config$network_type,
      log_transform = config$log_transform,
      effective_parameters = list(
        min_expression = config$min_expression,
        min_samples = config$min_samples,
        min_module_size = config$min_module_size,
        merge_cut_height = config$merge_cut_height,
        network_type = config$network_type,
        power = best_power,
        log_transform = config$log_transform,
        threads = config$threads
      )
    ),
    artifacts = list(
      list(
        artifact_id = "module-assignments",
        role = "module-assignments",
        kind = "table",
        path = file.path(config$output_directory, "module-assignments.csv"),
        format = "csv",
        media_type = "text/csv",
        size_bytes = unname(file.info(modules_path)$size),
        sha256 = digest::digest(file = modules_path, algo = "sha256", serialize = FALSE)
      ),
      list(
        artifact_id = "module-eigengenes",
        role = "module-eigengenes",
        kind = "table",
        path = file.path(config$output_directory, "module-eigengenes.csv"),
        format = "csv",
        media_type = "text/csv",
        size_bytes = unname(file.info(eigengenes_path)$size),
        sha256 = digest::digest(file = eigengenes_path, algo = "sha256", serialize = FALSE)
      ),
      list(
        artifact_id = "module-summary",
        role = "module-summary",
        kind = "table",
        path = file.path(config$output_directory, "module-summary.csv"),
        format = "csv",
        media_type = "text/csv",
        size_bytes = unname(file.info(summary_path)$size),
        sha256 = digest::digest(file = summary_path, algo = "sha256", serialize = FALSE)
      ),
      list(
        artifact_id = "scale-free-fit",
        role = "scale-free-fit",
        kind = "table",
        path = file.path(config$output_directory, "scale-free-fit.csv"),
        format = "csv",
        media_type = "text/csv",
        size_bytes = unname(file.info(scale_free_path)$size),
        sha256 = digest::digest(file = scale_free_path, algo = "sha256", serialize = FALSE)
      )
    ),
    provenance = list(
      engine_version = PACK_VERSION,
      execution_mode = "local-cpu",
      started_at = started_at,
      finished_at = format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC"),
      software = c(list(list(name = "R", version = paste(
        R.version$major, R.version$minor, sep = "."
      ))), loaded_software),
      input_sha256 = list(expression = digest::digest(
        file = config$expression_path, algo = "sha256", serialize = FALSE
      )),
      command = c("Rscript", "src/run_wgcna.R", "--request", "<request>", "--result", "<result>"),
      dependency_lock_sha256 = if (file.exists(lock_path)) {
        digest::digest(file = lock_path, algo = "sha256", serialize = FALSE)
      } else {
        ""
      }
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

validate_request <- function(document, result_path) {
  request <- require_object(document, "request")
  required_keys <- c("schema_version", "job_id", "capability", "inputs", "execution", "parameters")
  for (key in required_keys) {
    if (is.null(request[[key]])) {
      request_error(sprintf("request is missing: %s", key))
    }
  }
  if (!identical(request$schema_version, "2")) request_error("schema_version must be '2'")
  require_string(request$job_id, "job_id")
  capability <- require_string(request$capability, "capability")
  if (!capability %in% SUPPORTED_CAPABILITIES) {
    request_error(sprintf("capability must be one of: %s", paste(SUPPORTED_CAPABILITIES, collapse = ", ")))
  }
  execution <- require_object(request$execution, "execution")
  if (!identical(execution$mode, "local-cpu")) {
    request_error("execution.mode must be 'local-cpu'")
  }

  if (!is.list(request$inputs) || length(request$inputs) != 1L) {
    request_error("inputs must contain one expression artifact")
  }
  input <- request$inputs[[1L]]
  if (!is.list(input) || !identical(input$role, "expression")) {
    request_error("input must have role 'expression'")
  }
  if (!is.list(input$files) || length(input$files) != 1L) {
    request_error("input must contain exactly one file")
  }
  file_info <- input$files[[1L]]
  path <- require_string(file_info$path, "input file path")
  if (!file.exists(path) || dir.exists(path)) {
    request_error(sprintf("expression file does not exist: %s", path))
  }
  expression_path <- canonical_existing(path)

  parameters <- require_object(request$parameters, "parameters")
  output_directory <- require_string(parameters$output_directory, "parameters.output_directory")
  if (!dir.exists(dirname(output_directory))) {
    request_error(sprintf("output parent directory does not exist: %s", dirname(output_directory)))
  }
  if (file.exists(output_directory)) {
    request_error("parameters.output_directory must not already exist")
  }

  min_expression <- if (is.null(parameters$min_expression)) 1 else
    require_number(parameters$min_expression, "parameters.min_expression", 0, Inf)
  min_samples <- if (is.null(parameters$min_samples)) 3L else
    require_nonnegative_integer(parameters$min_samples, "parameters.min_samples")
  min_module_size <- if (is.null(parameters$min_module_size)) 30L else
    require_nonnegative_integer(parameters$min_module_size, "parameters.min_module_size")
  merge_cut_height <- if (is.null(parameters$merge_cut_height)) 0.25 else
    require_number(parameters$merge_cut_height, "parameters.merge_cut_height", 0, 1)
  network_type <- if (is.null(parameters$network_type)) "signed" else {
    nt <- require_string(parameters$network_type, "parameters.network_type")
    if (!nt %in% c("unsigned", "signed", "signed hybrid")) {
      request_error("network_type must be 'unsigned', 'signed', or 'signed hybrid'")
    }
    nt
  }
  power <- if (is.null(parameters$power)) 0 else
    require_nonnegative_integer(parameters$power, "parameters.power")
  log_transform <- if (is.null(parameters$log_transform)) TRUE else
    isTRUE(parameters$log_transform)
  threads <- if (is.null(parameters$threads)) 1L else
    require_nonnegative_integer(parameters$threads, "parameters.threads")
  if (threads < 1L) threads <- 1L

  list(
    job_id = request$job_id,
    capability = capability,
    expression_path = expression_path,
    output_directory = normalizePath(output_directory, winslash = "/", mustWork = FALSE),
    min_expression = min_expression,
    min_samples = min_samples,
    min_module_size = min_module_size,
    merge_cut_height = merge_cut_height,
    network_type = network_type,
    power = power,
    log_transform = log_transform,
    threads = threads
  )
}

parse_arguments <- function(arguments) {
  if (length(arguments) != 4L) stop("usage: run_wgcna.R --request REQUEST --result RESULT")
  request_index <- which(arguments == "--request")
  result_index <- which(arguments == "--result")
  if (length(request_index) != 1L || length(result_index) != 1L ||
      request_index == length(arguments) || result_index == length(arguments)) {
    stop("usage: run_wgcna.R --request REQUEST --result RESULT")
  }
  values <- c(request_index + 1L, result_index + 1L)
  flags <- c(request_index, result_index)
  if (!setequal(c(flags, values), seq_along(arguments))) {
    stop("unsupported or duplicate command-line arguments")
  }
  list(request = arguments[[request_index + 1L]], result = arguments[[result_index + 1L]])
}

main <- function() {
  started_at <- format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC")
  job_id <- "workflow-error"
  capability <- PRIMARY_CAPABILITY
  result_path <- NULL
  status <- tryCatch({
    options <- parse_arguments(commandArgs(trailingOnly = TRUE))
    result_path <- options$result
    project_library <- configure_project_library()
    if (!file.exists(options$request) || dir.exists(options$request)) {
      request_error(sprintf("request file does not exist: %s", options$request))
    }
    if (!requireNamespace("jsonlite", quietly = TRUE)) {
      stop("dependency jsonlite is not installed in the project library")
    }
    document <- jsonlite::fromJSON(options$request, simplifyVector = FALSE)
    if (is.list(document) && is.character(document$job_id) && length(document$job_id) == 1L &&
        !is.na(document$job_id) && nzchar(document$job_id)) {
      job_id <- document$job_id
    }
    if (is.list(document) && is.character(document$capability) &&
        length(document$capability) == 1L && !is.na(document$capability) &&
        document$capability %in% SUPPORTED_CAPABILITIES) {
      capability <- document$capability
    }
    config <- validate_request(document, options$result)
    capability <- config$capability
    check_runtime(project_library)
    result <- run_analysis(config, started_at, project_library)
    cat(jsonlite::toJSON(result, auto_unbox = TRUE, na = "null", null = "null", digits = NA), "\n")
    0L
  }, error = function(error) {
    payload <- minimal_error_json(job_id, capability, conditionMessage(error), started_at)
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