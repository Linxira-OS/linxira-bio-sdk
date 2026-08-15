#!/usr/bin/env Rscript

PACK_ID <- "org.linxira.medical-survival"
PACK_VERSION <- "0.1.0"
CAPABILITY <- "medical.survival.v1"
PREFERRED_R <- "4.6.1"
R_VERSION_REQUIREMENT <- ">=4.6.1,<4.7.0"
PACKAGE_REQUIREMENTS <- c(
  survival = ">=3.8.0,<3.9.0",
  jsonlite = ">=1.8.9,<3.0.0",
  digest = ">=0.6.37,<0.7.0"
)

core_version <- function() {
  value <- Sys.getenv("LINXIRA_BIO_CORE_VERSION", unset = "")
  if (nzchar(value)) value else "unknown"
}

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

configure_project_library <- function() {
  configured <- Sys.getenv("LINXIRA_BIO_WORKFLOW_R_LIBRARY", unset = "")
  if (!nzchar(configured)) {
    stop(paste0(
      "LINXIRA_BIO_WORKFLOW_R_LIBRARY must name the existing project-isolated ",
      "R package library (see bootstrap-survival-lib.R)"
    ))
  }
  library <- normalizePath(configured, winslash = "/", mustWork = FALSE)
  if (!dir.exists(library)) {
    stop(sprintf("project R library does not exist: %s", library))
  }
  .libPaths(c(library, .libPaths()))
  library
}

check_runtime <- function(project_library) {
  version <- paste(R.version$major, R.version$minor, sep = ".")
  if (!requireNamespace("survival", quietly = TRUE)) {
    stop(sprintf("dependency survival %s is not installed in the project library", PACKAGE_REQUIREMENTS[["survival"]]))
  }
  for (pkg in c("jsonlite", "digest")) {
    if (!requireNamespace(pkg, quietly = TRUE)) {
      stop(sprintf("dependency %s %s is not installed in the project library", pkg, PACKAGE_REQUIREMENTS[[pkg]]))
    }
  }
  if (!("survival" %in% rownames(installed.packages(lib.loc = project_library)))) {
    stop("dependency survival did not resolve from the project library")
  }
  list(version = version)
}

parse_arguments <- function(arguments) {
  if (length(arguments) != 4L) stop("usage: run_survival.R --request REQUEST --result RESULT")
  request_index <- which(arguments == "--request")
  result_index <- which(arguments == "--result")
  if (length(request_index) != 1L || length(result_index) != 1L ||
      request_index == length(arguments) || result_index == length(arguments)) {
    stop("usage: run_survival.R --request REQUEST --result RESULT")
  }
  values <- c(request_index + 1L, result_index + 1L)
  flags <- c(request_index, result_index)
  if (!setequal(c(flags, values), seq_along(arguments))) {
    stop("unsupported or duplicate command-line arguments")
  }
  list(request = arguments[[request_index + 1L]], result = arguments[[result_index + 1L]])
}

validate_request <- function(document, result_path) {
  config <- require_object(document, "request")
  require_string(config$schema_version, "schema_version")
  if (config$schema_version != "2") request_error("schema_version must be 2")
  require_string(config$job_id, "job_id")
  require_string(config$capability, "capability")
  if (config$capability != CAPABILITY) {
    request_error(sprintf("capability must be %s", CAPABILITY))
  }
  inputs <- config$inputs
  if (!is.list(inputs) || length(inputs) != 1L) {
    request_error("medical.survival.v1 requires exactly one input artifact")
  }
  input <- inputs[[1L]]
  require_string(input$role, "input role")
  if (input$role != "cohort") request_error("input role must be 'cohort'")
  files <- input$files
  if (!is.list(files) || length(files) != 1L) {
    request_error("input artifact must contain exactly one file")
  }
  cohort_path <- files[[1L]]$path
  require_string(cohort_path, "cohort file path")
  parameters <- require_object(config$parameters, "parameters")
  time_column <- require_string(parameters$time_column, "parameters.time_column")
  event_column <- require_string(parameters$event_column, "parameters.event_column")
  group_column <- require_string(parameters$group_column, "parameters.group_column")
  reference_level <- require_string(parameters$reference_level, "parameters.reference_level")
  output_directory <- require_string(parameters$output_directory, "parameters.output_directory")
  if (file.exists(output_directory) || dir.exists(output_directory)) {
    request_error(sprintf("refusing to overwrite workflow output directory: %s", output_directory))
  }
  list(
    job_id = config$job_id,
    capability = config$capability,
    cohort_path = cohort_path,
    time_column = time_column,
    event_column = event_column,
    group_column = group_column,
    reference_level = reference_level,
    output_directory = output_directory,
    result_path = result_path
  )
}

read_cohort <- function(config) {
  if (!file.exists(config$cohort_path)) {
    request_error(sprintf("cohort file does not exist: %s", config$cohort_path))
  }
  sep <- if (grepl("\\.tsv$|\\.tab$", config$cohort_path, ignore.case = TRUE)) "\t" else ","
  data <- tryCatch(
    utils::read.table(
      config$cohort_path, header = TRUE, sep = sep, check.names = FALSE,
      stringsAsFactors = FALSE, comment.char = "", fileEncoding = "UTF-8"
    ),
    error = function(error) request_error(sprintf("cannot parse cohort table: %s", conditionMessage(error)))
  )
  for (column in c(config$time_column, config$event_column, config$group_column)) {
    if (!(column %in% colnames(data))) {
      request_error(sprintf("cohort table lacks column %s", column))
    }
  }
  if (!is.numeric(data[[config$time_column]])) {
    request_error(sprintf("column %s must be numeric (survival time)", config$time_column))
  }
  if (!all(data[[config$event_column]] %in% c(0, 1))) {
    request_error(sprintf("column %s must contain only 0/1 event indicators", config$event_column))
  }
  if (!(config$reference_level %in% as.character(unique(data[[config$group_column]])))) {
    request_error(sprintf("reference_level %s is not present in column %s", config$reference_level, config$group_column))
  }
  data
}

run_analysis <- function(config, started_at, project_library) {
  data <- read_cohort(config)
  data[[config$group_column]] <- factor(
    data[[config$group_column]],
    levels = c(config$reference_level, setdiff(as.character(unique(data[[config$group_column]])), config$reference_level))
  )
  formula <- stats::as.formula(sprintf(
    "survival::Surv(%s, %s) ~ %s",
    config$time_column, config$event_column, config$group_column
  ))
  model <- survival::coxph(formula, data = data)
  summary_model <- summary(model)
  estimates <- summary_model$coefficients
  confidence <- summary_model$conf.int
  terms <- rownames(estimates)
  rows <- lapply(seq_along(terms), function(index) {
    list(
      term = terms[[index]],
      coefficient = unname(estimates[index, "coef"]),
      hazard_ratio = unname(confidence[index, "exp(coef)"]),
      standard_error = unname(estimates[index, "se(coef)"]),
      statistic = unname(estimates[index, "z"]),
      p_value = unname(estimates[index, "Pr(>|z|)"]),
      ci_low = unname(confidence[index, "lower .95"]),
      ci_high = unname(confidence[index, "upper .95"])
    )
  })
  fit <- survival::survfit(formula, data = data)
  summary_fit <- summary(fit)
  group_levels <- levels(data[[config$group_column]])
  km_rows <- lapply(seq_along(group_levels), function(index) {
    level <- group_levels[[index]]
    mask <- data[[config$group_column]] == level
    list(
      group = level,
      n = sum(mask),
      events = sum(data[[config$event_column]][mask]),
      median_survival = if (is.na(fit[index]$median)) NA_real_ else unname(fit[index]$median)
    )
  })
  cox_path <- file.path(config$output_directory, "cox-results.csv")
  km_path <- file.path(config$output_directory, "km-summary.csv")
  dir.create(config$output_directory, recursive = TRUE, showWarnings = FALSE)
  cox_table <- do.call(rbind, lapply(rows, function(row) data.frame(row, stringsAsFactors = FALSE)))
  utils::write.csv(cox_table, cox_path, row.names = FALSE, na = "")
  km_table <- do.call(rbind, lapply(km_rows, function(row) data.frame(row, stringsAsFactors = FALSE)))
  utils::write.csv(km_table, km_path, row.names = FALSE, na = "")
  if (!file.exists(config$result_path)) {
    request_error("result path directory does not exist")
  }
  list(
    schema_version = "2",
    job_id = config$job_id,
    capability = config$capability,
    status = "ok",
    result = list(
      n_observations = nrow(data),
      n_events = sum(data[[config$event_column]]),
      model_terms = terms,
      rows = rows,
      groups = km_rows,
      effective_parameters = list(
        time_column = config$time_column,
        event_column = config$event_column,
        group_column = config$group_column,
        reference_level = config$reference_level
      )
    ),
    artifacts = list(
      list(
        artifact_id = "cox-results",
        role = "cox-results",
        kind = "table",
        path = cox_path,
        format = "csv",
        media_type = "text/csv",
        size_bytes = unname(file.info(cox_path)$size),
        sha256 = digest::digest(file = cox_path, algo = "sha256", serialize = FALSE)
      ),
      list(
        artifact_id = "km-summary",
        role = "km-summary",
        kind = "table",
        path = km_path,
        format = "csv",
        media_type = "text/csv",
        size_bytes = unname(file.info(km_path)$size),
        sha256 = digest::digest(file = km_path, algo = "sha256", serialize = FALSE)
      )
    ),
    provenance = list(
      engine_version = PACK_VERSION,
      execution_mode = "local-cpu",
      core_version = core_version(),
      started_at = started_at,
      finished_at = format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC"),
      software = list(list(name = "R", version = paste(R.version$major, R.version$minor, sep = "."))),
      input_sha256 = list(cohort = digest::digest(file = config$cohort_path, algo = "sha256", serialize = FALSE)),
      command = c("Rscript", "src/run_survival.R", "--request", "<request>", "--result", "<result>"),
      dependency_lock_sha256 = digest::digest(
        file = file.path(SCRIPT_DIRECTORY, "dependencies.lock.json"),
        algo = "sha256", serialize = FALSE
      )
    ),
    diagnostics = list()
  )
}

minimal_error_json <- function(job_id, message, started_at) {
  sprintf(
    paste0(
      '{"schema_version":"2","job_id":%s,"capability":"%s","status":"error","result":{},',
      '"artifacts":[],"provenance":{"engine_version":"%s","execution_mode":"local-cpu",',
      '"core_version":"%s","started_at":"%s","finished_at":"%s"},"diagnostics":[{"code":"workflow_failed",',
      '"severity":"error","message":%s}]}'
    ),
    encodeString(job_id, quote = '"'), CAPABILITY, PACK_VERSION, core_version(),
    started_at, format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC"),
    encodeString(message, quote = '"')
  )
}

write_error_json_atomic <- function(result_path, payload) {
  target <- normalizePath(result_path, winslash = "/", mustWork = FALSE)
  parent <- dirname(target)
  if (!dir.exists(parent)) {
    dir.create(parent, recursive = TRUE, showWarnings = FALSE)
  }
  staging <- tempfile(pattern = ".result-", tmpdir = parent)
  writeLines(payload, staging, useBytes = TRUE)
  if (!file.rename(staging, target)) {
    stop("atomic result activation failed")
  }
}

arguments_all <- commandArgs(trailingOnly = FALSE)
file_argument <- grep("^--file=", arguments_all, value = TRUE)
SCRIPT_DIRECTORY <- if (length(file_argument) == 1L) {
  dirname(normalizePath(sub("^--file=", "", file_argument), winslash = "/", mustWork = TRUE))
} else {
  "."
}

main <- function() {
  started_at <- format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC")
  job_id <- "workflow-error"
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
    config <- validate_request(document, options$result)
    check_runtime(project_library)
    result <- run_analysis(config, started_at, project_library)
    dir.create(dirname(normalizePath(result_path, winslash = "/", mustWork = FALSE)),
               recursive = TRUE, showWarnings = FALSE)
    writeLines(jsonlite::toJSON(result, auto_unbox = TRUE, na = "null", null = "null", digits = NA),
               result_path, useBytes = TRUE)
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

if (sys.nframe() == 0L) quit(status = main(), save = "no")
