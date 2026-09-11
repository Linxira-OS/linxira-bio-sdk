#!/usr/bin/env Rscript
# Benchmark harness: the R backend of `linxira-bio benchmark run`.
#
# The worker invokes this pack when a request carries execution.backend = "r".
# It runs the independent R implementation of the requested capability (see
# implementations/), writes the same V2 result envelope the Rust engine
# produces, and self-reports the in-process wall time and peak RSS
# (/proc/self/status VmHWM) as an `info` diagnostic so the report can disclose
# interpreter start-up separately from the analysis (ROADMAP M2-T3, §7.1).

PACK_ID <- "org.linxira.benchmark-r"
PACK_VERSION <- "0.1.0"
BACKEND <- "r"
SELF_REPORTED_CODE <- "benchmark.self_reported"
HARNESS_PACKAGES <- c(jsonlite = ">=1.8.9,<3.0.0", digest = ">=0.6.37,<0.7.0")

core_version <- function() {
  value <- Sys.getenv("LINXIRA_BIO_CORE_VERSION", unset = "")
  if (nzchar(value)) value else "unknown"
}

utc_now <- function() format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC")

request_error <- function(message) {
  stop(structure(
    list(message = message, call = NULL),
    class = c("request_error", "error", "condition")
  ))
}

implementation_error <- function(message) {
  stop(structure(
    list(message = message, call = NULL),
    class = c("implementation_error", "error", "condition")
  ))
}

require_object <- function(value, context) {
  if (!is.list(value) || (length(value) > 0L && is.null(names(value)))) {
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

is_array <- function(value) is.list(value) && is.null(names(value))

same_path <- function(left, right) {
  canonical <- function(path) {
    path <- normalizePath(path, winslash = "/", mustWork = FALSE)
    # normalizePath leaves non-existent paths untouched, so unify separators
    # and trailing slashes by hand before comparing.
    sub("/+$", "", chartr("\\", "/", path))
  }
  identical(canonical(left), canonical(right))
}

# Benchmark policy: a project-isolated library is honoured when configured,
# otherwise the interpreter's own library paths are used, because the point of
# this backend is to measure the R stack as installed on the benchmark host.
# Whatever resolves is disclosed in provenance.software.
configure_library <- function() {
  configured <- Sys.getenv("LINXIRA_BIO_WORKFLOW_R_LIBRARY", unset = "")
  if (!nzchar(configured)) {
    return(invisible(NULL))
  }
  if (!dir.exists(configured)) {
    stop(sprintf("LINXIRA_BIO_WORKFLOW_R_LIBRARY names a missing directory: %s", configured))
  }
  library <- normalizePath(configured, winslash = "/", mustWork = TRUE)
  .libPaths(c(library, .libPaths()))
  invisible(library)
}

version_satisfies <- function(actual, requirement) {
  clauses <- trimws(strsplit(requirement, ",", fixed = TRUE)[[1L]])
  actual <- package_version(actual)
  for (clause in clauses) {
    operator <- regmatches(clause, regexpr("^(>=|<=|==|<|>)", clause))
    bound <- package_version(sub("^(>=|<=|==|<|>)", "", clause))
    ok <- switch(operator,
      ">=" = actual >= bound, "<=" = actual <= bound, "==" = actual == bound,
      "<" = actual < bound, ">" = actual > bound, FALSE
    )
    if (!ok) return(FALSE)
  }
  TRUE
}

check_packages <- function(requirements) {
  drift <- list()
  for (package in names(requirements)) {
    if (!requireNamespace(package, quietly = TRUE)) {
      stop(sprintf("dependency %s %s is not installed", package, requirements[[package]]))
    }
    installed <- as.character(utils::packageVersion(package))
    if (!version_satisfies(installed, requirements[[package]])) {
      drift[[length(drift) + 1L]] <- list(
        code = "dependency_version_drift",
        severity = "warning",
        message = sprintf(
          "%s %s is installed; the pack lock requires %s. Numbers remain comparable but the environment disclosure records the installed version.",
          package, installed, requirements[[package]]
        )
      )
    }
  }
  drift
}

sha256_file <- function(path) digest::digest(path, algo = "sha256", file = TRUE)

# Peak resident set size of this process in MiB, or NULL where the kernel
# does not expose it (Windows, macOS): the report then keeps the field null
# rather than inventing a figure.
peak_rss_mb <- function() {
  status <- "/proc/self/status"
  if (!file.exists(status)) return(NULL)
  lines <- readLines(status, warn = FALSE)
  line <- grep("^VmHWM:", lines, value = TRUE)
  if (length(line) != 1L) return(NULL)
  fields <- strsplit(trimws(sub("^VmHWM:", "", line)), "[[:space:]]+")[[1L]]
  kilobytes <- suppressWarnings(as.numeric(fields[[1L]]))
  if (is.na(kilobytes)) return(NULL)
  kilobytes / 1024
}

load_implementations <- function(directory) {
  files <- sort(list.files(directory, pattern = "\\.R$", full.names = TRUE))
  registry <- list()
  for (file in files) {
    environment <- new.env(parent = globalenv())
    sys.source(file, envir = environment)
    implementation <- get("IMPLEMENTATION", envir = environment)
    registry[[implementation$capability]] <- implementation
  }
  registry
}

validate_request <- function(document, result_path, registry) {
  request <- require_object(document, "request")
  if (!identical(request$schema_version, "2")) request_error("request schema_version must be \"2\"")
  job_id <- require_string(request$job_id, "job_id")
  capability <- require_string(request$capability, "capability")
  implementation <- registry[[capability]]
  if (is.null(implementation)) {
    request_error(sprintf(
      "%s has no r implementation of %s; supported: %s",
      PACK_ID, capability, paste(sort(names(registry)), collapse = ", ")
    ))
  }
  execution <- require_object(request$execution, "execution")
  backend <- if (is.null(execution$backend)) BACKEND else execution$backend
  if (!identical(backend, BACKEND)) {
    request_error(sprintf("execution.backend must be \"%s\", got \"%s\"", BACKEND, backend))
  }

  if (!is_array(request$inputs)) request_error("inputs must be an array")
  resolved <- list()
  declared_sha256 <- list()
  for (index in seq_along(request$inputs)) {
    artifact <- require_object(request$inputs[[index]], sprintf("inputs[%d]", index - 1L))
    role <- require_string(artifact$role, sprintf("inputs[%d].role", index - 1L))
    if (!role %in% implementation$input_roles) {
      request_error(sprintf("%s does not accept input role %s", capability, role))
    }
    if (!is.null(resolved[[role]])) request_error(sprintf("duplicate input role: %s", role))
    if (!identical(artifact$cardinality, "single")) {
      request_error(sprintf("input role %s requires cardinality \"single\"", role))
    }
    if (!is_array(artifact$files) || length(artifact$files) != 1L) {
      request_error(sprintf("input role %s requires exactly one file", role))
    }
    file <- require_object(artifact$files[[1L]], sprintf("inputs[%d].files[0]", index - 1L))
    path <- require_string(file$path, sprintf("inputs[%d].files[0].path", index - 1L))
    if (!file.exists(path) || dir.exists(path)) request_error(sprintf("input file does not exist: %s", path))
    compression <- if (is.null(file$compression)) "none" else file$compression
    if (!identical(compression, "none")) {
      request_error("the benchmark harness reads uncompressed inputs only")
    }
    sha <- file$sha256
    if (!is.null(sha) && (!is.character(sha) || length(sha) != 1L || nchar(sha) != 64L)) {
      request_error(sprintf("inputs[%d].files[0].sha256 must be a 64-character hex string", index - 1L))
    }
    resolved[[role]] <- normalizePath(path, winslash = "/", mustWork = TRUE)
    declared_sha256[[role]] <- if (is.null(sha)) NA_character_ else tolower(sha)
  }
  missing <- setdiff(implementation$input_roles, names(resolved))
  if (length(missing) > 0L) {
    request_error(sprintf("%s requires inputs: %s", capability, paste(missing, collapse = ", ")))
  }

  parameters <- request$parameters
  if (is.null(parameters)) parameters <- list()
  parameters <- require_object(parameters, "parameters")
  output_directory <- require_string(parameters$output_directory, "parameters.output_directory")
  if (!grepl("^(/|[A-Za-z]:[/\\\\]|\\\\\\\\)", output_directory)) {
    request_error("parameters.output_directory must be an absolute path")
  }
  for (name in names(parameters)) {
    if (name != "output_directory" && !name %in% implementation$parameters) {
      request_error(sprintf("%s does not accept parameter %s", capability, name))
    }
  }
  if (!same_path(dirname(result_path), output_directory)) {
    request_error("the result path must live directly inside parameters.output_directory")
  }
  if (!dir.exists(dirname(output_directory))) {
    request_error(sprintf("output parent directory does not exist: %s", dirname(output_directory)))
  }
  for (path in resolved) {
    if (same_path(path, output_directory)) request_error("output directory must differ from every input")
  }

  list(
    job_id = job_id,
    capability = capability,
    implementation = implementation,
    inputs = resolved,
    declared_sha256 = declared_sha256,
    parameters = parameters[setdiff(names(parameters), "output_directory")],
    output_directory = output_directory,
    result_path = result_path
  )
}

write_json_file <- function(path, document) {
  jsonlite::write_json(
    document, path,
    auto_unbox = TRUE, null = "null", na = "null", digits = NA, pretty = TRUE
  )
}

run_benchmarked <- function(config, started_at) {
  implementation <- config$implementation
  capability <- config$capability
  output_directory <- config$output_directory
  drift <- check_packages(implementation$packages)

  # The worker already verified every declared hash and re-checks the inputs
  # after the pack exits; the declared digest is reused so a multi-terabyte
  # input is not hashed a third time inside this backend's wall time.
  input_sha256 <- list()
  for (role in names(config$inputs)) {
    declared <- config$declared_sha256[[role]]
    input_sha256[[role]] <- if (is.na(declared)) sha256_file(config$inputs[[role]]) else declared
  }

  # Instrumentation brackets the analysis only; interpreter start-up,
  # validation and output writing are visible to the outer timer instead.
  started <- Sys.time()
  result <- implementation$run(config$inputs, config$parameters)
  wall_ms <- as.numeric(difftime(Sys.time(), started, units = "secs")) * 1000
  peak <- peak_rss_mb()

  software <- c(
    list(list(name = "R", version = paste(R.version$major, R.version$minor, sep = "."))),
    implementation$software(),
    lapply(names(HARNESS_PACKAGES), function(package) {
      list(name = package, version = as.character(utils::packageVersion(package)), package_id = paste0("cran-", package))
    })
  )
  diagnostics <- c(drift, list(list(
    code = SELF_REPORTED_CODE,
    severity = "info",
    message = as.character(jsonlite::toJSON(
      list(
        wall_ms = round(wall_ms, 3),
        peak_rss_mb = if (is.null(peak)) NULL else round(peak, 3),
        instrument = "Sys.time + /proc/self/status VmHWM",
        backend = BACKEND
      ),
      auto_unbox = TRUE, null = "null", digits = NA
    ))
  )))

  staging <- tempfile(pattern = ".linxira-benchmark-", tmpdir = dirname(output_directory))
  if (!dir.create(staging, mode = "0700")) stop("could not create the staging directory")
  committed <- FALSE
  on.exit(if (!committed && dir.exists(staging)) unlink(staging, recursive = TRUE, force = TRUE), add = TRUE)

  artifact_name <- paste0(capability, ".result.json")
  staged_artifact <- file.path(staging, artifact_name)
  write_json_file(staged_artifact, result)
  lock_path <- file.path(SCRIPT_DIRECTORY, "..", "dependencies.lock.json")
  envelope <- list(
    schema_version = "2",
    job_id = config$job_id,
    capability = capability,
    status = "ok",
    result = result,
    artifacts = list(list(
      artifact_id = "benchmark-result",
      role = "benchmark-result",
      kind = "report",
      path = file.path(output_directory, artifact_name),
      format = "json",
      media_type = "application/json",
      size_bytes = as.numeric(file.info(staged_artifact)$size),
      sha256 = sha256_file(staged_artifact)
    )),
    provenance = list(
      engine_version = PACK_VERSION,
      execution_mode = "local-cpu",
      core_version = core_version(),
      started_at = started_at,
      finished_at = utc_now(),
      software = software,
      input_sha256 = input_sha256,
      command = list("Rscript", "src/benchmark_harness.R", "--request", "<request>", "--result", "<result>"),
      dependency_lock_sha256 = sha256_file(normalizePath(lock_path, winslash = "/", mustWork = TRUE))
    ),
    diagnostics = diagnostics
  )
  write_json_file(file.path(staging, basename(config$result_path)), envelope)
  if (dir.exists(output_directory) || file.exists(output_directory)) {
    request_error("output directory appeared while the analysis was running")
  }
  if (!file.rename(staging, output_directory)) stop("could not activate the output directory")
  committed <- TRUE
  envelope
}

minimal_error_json <- function(job_id, capability, message, started_at) {
  sprintf(
    paste0(
      '{"schema_version":"2","job_id":%s,"capability":%s,"status":"error",',
      '"result":{},"artifacts":[],"provenance":{"engine_version":"%s",',
      '"execution_mode":"local-cpu","core_version":"%s",',
      '"started_at":"%s","finished_at":"%s"},',
      '"diagnostics":[{"code":"workflow_failed","severity":"error","message":%s}]}'
    ),
    encodeString(job_id, quote = '"'), encodeString(capability, quote = '"'),
    PACK_VERSION, core_version(), started_at, utc_now(),
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

parse_arguments <- function(arguments) {
  usage <- "usage: benchmark_harness.R --request REQUEST --result RESULT"
  if (length(arguments) != 4L) stop(usage)
  request_index <- which(arguments == "--request")
  result_index <- which(arguments == "--result")
  if (length(request_index) != 1L || length(result_index) != 1L ||
      request_index == length(arguments) || result_index == length(arguments)) {
    stop(usage)
  }
  if (!setequal(c(request_index, result_index, request_index + 1L, result_index + 1L), seq_along(arguments))) {
    stop("unsupported or duplicate command-line arguments")
  }
  list(request = arguments[[request_index + 1L]], result = arguments[[result_index + 1L]])
}

main <- function(arguments = commandArgs(trailingOnly = TRUE)) {
  started_at <- utc_now()
  job_id <- "workflow-error"
  capability <- "unknown"
  result_path <- NULL
  tryCatch({
    options <- parse_arguments(arguments)
    result_path <- options$result
    configure_library()
    if (!file.exists(options$request) || dir.exists(options$request)) {
      request_error(sprintf("request file does not exist: %s", options$request))
    }
    if (same_path(options$request, options$result)) {
      request_error("result path must not alias the request file")
    }
    check_packages(HARNESS_PACKAGES)
    document <- jsonlite::fromJSON(options$request, simplifyVector = FALSE)
    if (is.list(document)) {
      if (is.character(document$job_id) && length(document$job_id) == 1L && nzchar(document$job_id)) {
        job_id <- document$job_id
      }
      if (is.character(document$capability) && length(document$capability) == 1L && nzchar(document$capability)) {
        capability <- document$capability
      }
    }
    registry <- load_implementations(file.path(SCRIPT_DIRECTORY, "implementations"))
    config <- validate_request(document, options$result, registry)
    run_benchmarked(config, started_at)
    0L
  }, error = function(error) {
    payload <- minimal_error_json(job_id, capability, conditionMessage(error), started_at)
    if (!is.null(result_path)) {
      try(write_error_json_atomic(result_path, payload), silent = TRUE)
    }
    message(sprintf("%s: %s", PACK_ID, conditionMessage(error)))
    2L
  })
}

arguments_all <- commandArgs(trailingOnly = FALSE)
file_argument <- grep("^--file=", arguments_all, value = TRUE)
SCRIPT_DIRECTORY <- if (length(file_argument) == 1L) {
  dirname(normalizePath(sub("^--file=", "", file_argument), winslash = "/", mustWork = TRUE))
} else {
  getwd()
}

if (sys.nframe() == 0L) quit(status = main(), save = "no")
