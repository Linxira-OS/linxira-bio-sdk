#!/usr/bin/env Rscript
# Strict, local-only ComBat batch-effect correction (parametric empirical Bayes).
#
# Reference: Johnson, Li & Rabinovic, "Adjusting batch effects in microarray
# expression data using empirical Bayes methods", Biostatistics 8(1), 2007,
# doi:10.1093/biostatistics/kxj037. This pack implements the parametric
# posterior-mode variant with an alternating fixed-point iteration; the Python
# pack org.linxira.batch-combat-py implements the identical specification and
# the two backends are held to byte-identical TSV output on the same inputs.
#
# Usage: Rscript run_combat.R --request <request.json> --result <result.json>

suppressPackageStartupMessages(library(jsonlite))

PACK_ID <- "org.linxira.batch-combat-r"
PACK_VERSION <- "0.1.0"
CAPABILITY <- "expression.batch-correct.v1"
METHOD <- "combat-parametric"
MISSING_TOKENS <- c("", "na", "nan", "null", "none")
MAX_ITERATIONS <- 5000L
CONVERGENCE_TOLERANCE <- 1e-8

strip_extended_prefix <- function(path) {
  # Windows canonical paths carry the extended-length prefix "\\?\", which
  # R's file API does not open; drop it back to a plain absolute path.
  if (startsWith(path, "\\\\?\\")) substring(path, 5L) else path
}

request_error <- function(message) {
  structure(list(message = message), class = c("request_error", "error", "condition"))
}

utc_now <- function() {
  format(Sys.time(), "%Y-%m-%dT%H:%M:%SZ", tz = "UTC")
}

core_version <- function() {
  value <- Sys.getenv("LINXIRA_BIO_CORE_VERSION", unset = "unknown")
  if (nzchar(value)) value else "unknown"
}

sha256_file <- function(path) {
  as.character(tools::sha256sum(path))
}

read_request <- function(path) {
  text <- tryCatch(readChar(path, file.info(path)$size, useBytes = TRUE),
    error = function(error) stop(request_error(paste0("cannot read request: ", conditionMessage(error))))
  )
  request <- tryCatch(fromJSON(text, simplifyVector = FALSE),
    error = function(error) stop(request_error(paste0("cannot parse request: ", conditionMessage(error))))
  )
  if (!is.list(request)) stop(request_error("request must be an object"))
  request
}

artifact_files <- function(request, role) {
  inputs <- request$inputs
  if (!is.list(inputs)) stop(request_error("request inputs must be a list"))
  matches <- Filter(function(artifact) {
    is.list(artifact) && identical(artifact$role, role)
  }, inputs)
  if (length(matches) != 1L) {
    stop(request_error(sprintf("request must contain exactly one %s input artifact", role)))
  }
  files <- matches[[1]]$files
  if (!is.list(files) || length(files) != 1L) {
    stop(request_error(sprintf("%s artifact must contain exactly one file", role)))
  }
  path <- files[[1]]$path
  if (!is.character(path) || !nzchar(path)) {
    stop(request_error(sprintf("%s file path is missing", role)))
  }
  # Windows canonical paths carry the extended-length prefix "\\?\", which
  # R's file API does not open; strip it back to a plain absolute path.
  strip_extended_prefix(path)
}

delimiter_for <- function(path, label) {
  extension <- tolower(tools::file_ext(path))
  if (extension %in% c("tsv", "txt")) return("\t")
  if (extension == "csv") return(",")
  stop(request_error(sprintf("%s must be .csv or .tsv: %s", label, basename(path))))
}

check_identifier <- function(value, label) {
  if (grepl("[\t\r\n]", value)) {
    stop(request_error(sprintf("%s contains a delimiter character: %s", label, value)))
  }
  value
}

parse_float <- function(value, label) {
  token <- trimws(value)
  if (tolower(token) %in% MISSING_TOKENS) {
    stop(request_error(sprintf("missing value in %s: \"%s\"", label, token)))
  }
  number <- suppressWarnings(as.numeric(token))
  if (is.na(number) || !is.finite(number)) {
    stop(request_error(sprintf("%s is not numeric: \"%s\"", label, token)))
  }
  number
}

split_lines_strict <- function(text, label) {
  lines <- strsplit(text, "\r\n|\r|\n", fixed = FALSE)[[1]]
  lines <- lines[nzchar(lines)]
  if (length(lines) == 0L) stop(request_error(sprintf("%s is empty: %s", label, basename(path))))
  lines
}

read_table <- function(path, label) {
  delimiter <- delimiter_for(path, label)
  text <- tryCatch(readChar(path, file.info(path)$size, useBytes = TRUE),
    error = function(error) stop(request_error(paste0("cannot read ", label, ": ", conditionMessage(error))))
  )
  lines <- strsplit(text, "\r\n|\r|\n", fixed = FALSE)[[1]]
  lines <- lines[nzchar(lines)]
  if (length(lines) == 0L) stop(request_error(sprintf("%s is empty: %s", label, basename(path))))
  rows <- strsplit(lines, delimiter, fixed = TRUE)
  width <- length(rows[[1]])
  if (width < 2L) {
    stop(request_error(sprintf("%s must have a header and at least one data column", label)))
  }
  for (index in seq_along(rows)) {
    if (length(rows[[index]]) != width) {
      stop(request_error(sprintf(
        "%s row %d has %d fields, expected %d",
        label, index, length(rows[[index]]), width
      )))
    }
  }
  rows
}

read_matrix <- function(path) {
  rows <- read_table(path, "expression matrix")
  header <- vapply(rows[[1]], function(cell) check_identifier(trimws(cell), "sample id"), "")
  samples <- header[-1L]
  if (anyDuplicated(samples)) stop(request_error("expression matrix has duplicate sample ids"))
  feature_ids <- character(length(rows) - 1L)
  values <- matrix(0, nrow = length(rows) - 1L, ncol = length(samples),
                   dimnames = list(NULL, samples))
  for (row_index in seq_along(feature_ids)) {
    row <- rows[[row_index + 1L]]
    feature_ids[row_index] <- check_identifier(trimws(row[1]), "feature id")
    for (column_index in seq_along(samples)) {
      values[row_index, column_index] <- parse_float(
        row[column_index + 1L],
        sprintf("expression value for %s/%s", feature_ids[row_index], samples[column_index])
      )
    }
  }
  if (length(feature_ids) == 0L) stop(request_error("expression matrix contains no features"))
  list(features = feature_ids, samples = samples, values = values)
}

read_sample_table <- function(path, batch_column) {
  rows <- read_table(path, "sample table")
  header <- trimws(rows[[1]])
  if (!(batch_column %in% header)) {
    stop(request_error(sprintf("sample table has no column named \"%s\"", batch_column)))
  }
  sample_index <- 1L
  batch_index <- which(header == batch_column)
  if (batch_index == sample_index) {
    stop(request_error("batch column must not be the sample-id column"))
  }
  covariate_indices <- setdiff(2L:length(header), c(sample_index, batch_index))
  sample_ids <- character(length(rows) - 1L)
  if (anyDuplicated(trimws(vapply(rows[-1L], function(row) row[sample_index], "")))) {
    stop(request_error("sample table has duplicate sample ids"))
  }
  batches <- character(length(rows) - 1L)
  covariate_columns <- vector("list", length(covariate_indices))
  for (row_index in seq_along(sample_ids)) {
    row <- rows[[row_index + 1L]]
    sample_ids[row_index] <- check_identifier(trimws(row[sample_index]), "sample id")
    batch <- trimws(row[batch_index])
    if (tolower(batch) %in% MISSING_TOKENS) {
      stop(request_error(sprintf("missing batch label for sample \"%s\"", sample_ids[row_index])))
    }
    batches[row_index] <- batch
    for (position in seq_along(covariate_indices)) {
      covariate_columns[[position]][row_index] <- trimws(row[covariate_indices[position]])
    }
  }
  covariate_names <- header[covariate_indices]
  numeric_columns <- lapply(seq_along(covariate_columns), function(position) {
    raw <- covariate_columns[[position]]
    missing_positions <- which(tolower(raw) %in% MISSING_TOKENS)
    if (length(missing_positions) > 0L) {
      stop(request_error(sprintf(
        "missing value in covariate \"%s\" for sample \"%s\"",
        covariate_names[position], sample_ids[missing_positions[1]]
      )))
    }
    looks_numeric <- !any(is.na(suppressWarnings(as.numeric(raw))))
    if (looks_numeric) {
      vapply(raw, function(value) parse_float(value, sprintf("covariate \"%s\"", covariate_names[position])), 0)
    } else {
      levels <- sort(unique(raw))
      if (length(levels) != 2L) {
        stop(request_error(sprintf(
          "covariate \"%s\" must be numeric or have exactly two levels (found %d)",
          covariate_names[position], length(levels)
        )))
      }
      vapply(raw, function(value) if (identical(value, levels[1])) 0 else 1, 0)
    }
  })
  list(
    sample_ids = sample_ids,
    batches = batches,
    covariate_names = as.character(covariate_names),
    covariate_values = numeric_columns
  )
}

combat_adjust <- function(values, batches, covariates) {
  sample_count <- ncol(values)
  labels <- sort(unique(batches))
  if (length(labels) < 2L) {
    stop(request_error("batch correction requires at least two batches"))
  }
  counts <- vapply(labels, function(label) sum(batches == label), 0)
  if (any(counts < 2L)) {
    offender <- labels[which(counts < 2L)[1]]
    stop(request_error(sprintf(
      "batch \"%s\" has %d sample(s); at least two are required",
      offender, counts[[offender]]
    )))
  }
  design <- matrix(0, nrow = sample_count, ncol = length(labels) + ncol(covariates))
  for (column in seq_along(labels)) {
    design[, column] <- as.numeric(batches == labels[column])
  }
  if (ncol(covariates) > 0L) design[, (length(labels) + 1L):ncol(design)] <- as.matrix(covariates)
  if (qr(design)$rank < ncol(design)) {
    stop(request_error("batch design matrix is rank deficient (covariates collinear with batch)"))
  }
  coefficients <- solve(crossprod(design), crossprod(design, t(values)))
  fitted <- t(design %*% coefficients)
  residuals <- values - fitted
  fractions <- counts / sample_count
  grand_mean <- as.numeric(crossprod(fractions, coefficients[seq_along(labels), , drop = FALSE]))
  covariate_part <- t(as.matrix(covariates) %*% coefficients[(length(labels) + 1L):nrow(coefficients), , drop = FALSE])
  pooled_variance <- rowMeans(residuals^2)
  sigma <- sqrt(pooled_variance)
  if (any(sigma <= 0)) {
    offenders <- head(which(sigma <= 0), 5)
    stop(request_error(sprintf(
      "features with zero residual variance cannot be corrected: rows %s",
      paste(offenders, collapse = ", ")
    )))
  }
  standardized <- (values - grand_mean - covariate_part) / sigma

  gamma_hat <- matrix(0, nrow = length(labels), ncol = nrow(values))
  delta_hat_squared <- matrix(0, nrow = length(labels), ncol = nrow(values))
  for (column in seq_along(labels)) {
    indices <- which(batches == labels[column])
    gamma_hat[column, ] <- rowMeans(standardized[, indices, drop = FALSE])
    delta_hat_squared[column, ] <- apply(standardized[, indices, drop = FALSE], 1, var)
  }

  gamma_bar <- rowMeans(gamma_hat)
  tau_squared <- apply(gamma_hat, 1, var)
  delta_bar <- rowMeans(delta_hat_squared)
  delta_variance <- apply(delta_hat_squared, 1, var)
  a_prior <- (2 * delta_variance + delta_bar^2) / delta_variance
  b_prior <- (delta_bar * delta_variance + delta_bar^3) / delta_variance

  iterations_used <- 0L
  gamma_star <- gamma_hat
  delta_star <- delta_hat_squared
  for (column in seq_along(labels)) {
    indices <- which(batches == labels[column])
    count <- as.numeric(counts[column])
    for (feature in seq_len(nrow(values))) {
      observations <- standardized[feature, indices]
      batch_mean <- gamma_hat[column, feature]
      batch_variance <- delta_hat_squared[column, feature]
      gamma <- batch_mean
      delta_squared <- batch_variance
      converged <- FALSE
      for (iteration in seq_len(MAX_ITERATIONS)) {
        denominator <- count * tau_squared[column] + delta_squared
        if (denominator <= 0) {
          stop(request_error(sprintf("degenerate empirical-Bayes prior for batch \"%s\"", labels[column])))
        }
        gamma_new <- (count * tau_squared[column] * batch_mean + delta_squared * gamma_bar[column]) / denominator
        sum_squares <- sum((observations - gamma_new)^2)
        delta_new <- (0.5 * sum_squares + b_prior[column]) / (count / 2 + a_prior[column] - 1)
        gamma_change <- abs(gamma_new - gamma) / (1 + abs(gamma))
        delta_change <- abs(delta_new - delta_squared) / (1 + abs(delta_squared))
        gamma <- gamma_new
        delta_squared <- delta_new
        if (gamma_change < CONVERGENCE_TOLERANCE && delta_change < CONVERGENCE_TOLERANCE) {
          iterations_used <- max(iterations_used, iteration)
          converged <- TRUE
          break
        }
      }
      if (!converged) {
        stop(request_error(sprintf(
          "empirical-Bayes iteration did not converge for batch \"%s\" within %d iterations",
          labels[column], MAX_ITERATIONS
        )))
      }
      gamma_star[column, feature] <- gamma
      delta_star[column, feature] <- delta_squared
    }
  }

  adjusted <- matrix(0, nrow = nrow(values), ncol = sample_count)
  for (column in seq_along(labels)) {
    indices <- which(batches == labels[column])
    adjusted[, indices] <- sigma * (standardized[, indices, drop = FALSE] - gamma_star[column, ]) /
      sqrt(delta_star[column, ]) + grand_mean + covariate_part[, indices, drop = FALSE]
  }
  list(
    adjusted = adjusted,
    iterations_used = iterations_used,
    priors = list(
      gamma_bar = as.list(setNames(as.numeric(gamma_bar), labels)),
      tau_squared = as.list(setNames(as.numeric(tau_squared), labels)),
      a_prior = as.list(setNames(as.numeric(a_prior), labels)),
      b_prior = as.list(setNames(as.numeric(b_prior), labels))
    )
  )
}

write_corrected_matrix <- function(path, features, samples, adjusted) {
  lines <- character(length(features) + 1L)
  lines[1] <- paste(c("feature", samples), collapse = "\t")
  for (row_index in seq_along(features)) {
    cells <- c(features[row_index], sprintf("%.10g", adjusted[row_index, ]))
    lines[row_index + 1L] <- paste(cells, collapse = "\t")
  }
  writeLines(lines, con = path, useBytes = TRUE)
}

script_pack_root <- function() {
  file_argument <- grep("^--file=", commandArgs(trailingOnly = FALSE), value = TRUE)
  if (length(file_argument) != 1L) {
    stop(request_error("cannot locate the pack root (script was not invoked via --file)"))
  }
  script_path <- sub("^--file=", "", file_argument)
  normalizePath(file.path(dirname(script_path), ".."), mustWork = FALSE)
}

success_result <- function(job_id, output_path, started_at, input_sha256, summary, pack_root) {
  lock_path <- file.path(pack_root, "dependencies.lock.json")
  list(
    schema_version = "2",
    job_id = job_id,
    capability = CAPABILITY,
    status = "ok",
    result = list(
      method = METHOD,
      feature_count = summary$feature_count,
      sample_count = summary$sample_count,
      batch_count = length(summary$batch_counts),
      batch_counts = summary$batch_counts,
      covariate_columns = as.character(summary$covariate_names),
      eb_iterations_max = as.integer(summary$iterations_used),
      eb_priors = summary$priors
    ),
    artifacts = list(list(
      artifact_id = "corrected-matrix",
      role = "corrected-expression",
      kind = "domain-file",
      path = output_path,
      format = "tsv",
      media_type = "text/tab-separated-values",
      size_bytes = as.integer(file.info(output_path)$size),
      sha256 = sha256_file(output_path)
    )),
    provenance = list(
      engine_version = PACK_VERSION,
      execution_mode = "local-cpu",
      core_version = core_version(),
      started_at = started_at,
      finished_at = utc_now(),
      software = list(list(name = "R", version = paste(R.version$major, R.version$minor, sep = "."))),
      input_sha256 = input_sha256,
      command = list("Rscript", "src/run_combat.R", "--request", "<request>", "--result", "<result>"),
      dependency_lock_sha256 = sha256_file(lock_path)
    ),
    diagnostics = list()
  )
}

error_result <- function(job_id, message, started_at) {
  list(
    schema_version = "2",
    job_id = job_id,
    capability = CAPABILITY,
    status = "error",
    result = list(),
    artifacts = list(),
    provenance = list(
      engine_version = PACK_VERSION,
      execution_mode = "local-cpu",
      core_version = core_version(),
      started_at = started_at,
      finished_at = utc_now()
    ),
    diagnostics = list(list(code = "workflow_failed", severity = "error", message = message))
  )
}

parse_arguments <- function(arguments) {
  arguments <- as.character(arguments)
  request <- NULL
  result <- NULL
  index <- 1L
  while (index <= length(arguments)) {
    if (identical(arguments[index], "--request")) {
      index <- index + 1L
      request <- arguments[index]
    } else if (identical(arguments[index], "--result")) {
      index <- index + 1L
      result <- arguments[index]
    } else {
      stop(request_error(sprintf("unknown argument: %s", arguments[index])))
    }
    index <- index + 1L
  }
  if (is.null(request) || is.null(result)) {
    stop(request_error("usage: run_combat.R --request <request.json> --result <result.json>"))
  }
  list(request = request, result = result)
}

main <- function(arguments) {
  options <- parse_arguments(arguments)
  started_at <- utc_now()
  pack_root <- script_pack_root()
  request <- list()
  job_id <- "unknown"
  result <- tryCatch({
    request <- read_request(options$request)
    job_id <- request$job_id
    if (!is.character(job_id) || !nzchar(job_id)) stop(request_error("job_id is required"))
    method <- if (is.list(request$parameters) && !is.null(request$parameters$method)) request$parameters$method else "combat"
    if (!identical(method, "combat")) {
      stop(request_error(sprintf("unsupported batch-correction method \"%s\"; expected 'combat'", method)))
    }
    matrix_path <- artifact_files(request, "expression-matrix")
    sample_table_path <- artifact_files(request, "sample-metadata")
    if (!file.exists(matrix_path)) {
      stop(request_error(sprintf("expression-matrix file does not exist: %s", matrix_path)))
    }
    if (!file.exists(sample_table_path)) {
      stop(request_error(sprintf("sample-metadata file does not exist: %s", sample_table_path)))
    }
    parameters <- if (is.list(request$parameters)) request$parameters else list()
    batch_column <- if (!is.null(parameters$batch_column) && nzchar(parameters$batch_column)) {
      trimws(parameters$batch_column)
    } else {
      "batch"
    }
    output_directory <- parameters$output_directory
    if (!is.character(output_directory) || !nzchar(output_directory)) {
      stop(request_error("parameters.output_directory is required"))
    }
    output_directory <- strip_extended_prefix(output_directory)
    output_filename <- if (!is.null(parameters$output_filename) && nzchar(parameters$output_filename)) {
      parameters$output_filename
    } else {
      "corrected.tsv"
    }
    output_path <- file.path(output_directory, output_filename)
    matrix <- read_matrix(matrix_path)
    table <- read_sample_table(sample_table_path, batch_column)
    if (setequal(table$sample_ids, matrix$samples) && length(table$sample_ids) == length(matrix$samples)) {
      order <- match(matrix$samples, table$sample_ids)
    } else {
      stop(request_error("sample table ids do not match the expression matrix columns exactly"))
    }
    batches <- table$batches[order]
    covariates <- if (length(table$covariate_names) > 0L) {
      as.matrix(do.call(cbind, lapply(table$covariate_values, function(column) column[order])))
    } else {
      matrix(0, nrow = length(matrix$samples), ncol = 0)
    }
    dir.create(dirname(output_path), recursive = TRUE, showWarnings = FALSE)
    adjustment <- combat_adjust(matrix$values, batches, covariates)
    write_corrected_matrix(output_path, matrix$features, matrix$samples, adjustment$adjusted)
    batch_counts <- as.list(table(batches))
    success_result(job_id, output_path, started_at, list(
      "expression-matrix" = sha256_file(matrix_path),
      "sample-metadata" = sha256_file(sample_table_path)
    ), list(
      feature_count = length(matrix$features),
      sample_count = length(matrix$samples),
      batch_counts = batch_counts,
      covariate_names = table$covariate_names,
      iterations_used = adjustment$iterations_used,
      priors = adjustment$priors
    ), pack_root)
  }, request_error = function(error) {
    error_result(job_id, error$message, started_at)
  }, error = function(error) {
    error_result(job_id, conditionMessage(error), started_at)
  })
  result_directory <- dirname(options$result)
  if (!dir.exists(result_directory)) dir.create(result_directory, recursive = TRUE, showWarnings = FALSE)
  writeLines(toJSON(result, auto_unbox = TRUE, digits = NA, null = "null"), con = options$result, useBytes = TRUE)
  cat(toJSON(result, auto_unbox = TRUE, digits = NA, null = "null"), "\n", sep = "")
  if (identical(result$status, "ok")) 0L else 2L
}

# Run main only when invoked directly via Rscript (sys.nframe() == 0);
# tests and interactive sessions source this file and call main() themselves.
if (sys.nframe() == 0L) {
  invisible(main(commandArgs(trailingOnly = TRUE)))
}
