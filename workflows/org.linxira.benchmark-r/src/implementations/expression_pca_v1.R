# expression.pca.v1 — independent base-R implementation (no extra packages).
#
# A line-for-line port of the Rust engine's PCA
# (engine/crates/linxira-bio-core/src/expression.rs): same matrix reader
# rules, centering and optional scaling, the same deterministic power
# iteration on the sample Gram matrix (sinusoidal start vector, Gram-Schmidt
# against earlier components, identical convergence thresholds), and the same
# sign convention — the element with the largest absolute value is made
# positive (Rust's `max_by` returns the LAST maximum on ties, so the scan
# keeps the last one). prcomp is deliberately not used: its SVD sign
# convention and rounding differ, and this backend exists for a
# field-for-field comparison against the engine.

IMPLEMENTATION <- list(
  capability = "expression.pca.v1",
  input_roles = c("matrix"),
  parameters = c("components", "scale_features"),
  packages = character(0),
  software = function() list(),
  run = function(inputs, parameters) {
    components_requested <- as.integer(parameters$components %||% 2)
    if (is.na(components_requested) || components_requested <= 0) {
      implementation_error("components must be at least 1")
    }
    scale_features <- isTRUE(parameters$scale_features)
    matrix_data <- read_pca_matrix(inputs[["matrix"]])
    if (length(matrix_data$samples) < 2L) {
      implementation_error("PCA requires at least two samples")
    }
    centered <- centered_rows(matrix_data$values, scale_features)
    denominator <- length(matrix_data$samples) - 1
    total_variance <- sum(vapply(
      centered$rows,
      function(row) sum(row * row),
      numeric(1)
    )) / denominator
    if (total_variance <= .Machine$double.eps) {
      implementation_error("PCA requires at least one feature with non-zero variance")
    }
    component_limit <- min(components_requested, length(matrix_data$samples) - 1L, length(matrix_data$features))
    eigenpairs <- leading_eigenpairs(centered$rows, component_limit, denominator)
    if (length(eigenpairs) == 0L) {
      implementation_error("PCA could not resolve a non-zero component")
    }

    sample_scores <- setNames(
      vector("list", length(matrix_data$samples)),
      matrix_data$samples
    )
    components <- vector("list", length(eigenpairs))
    for (index in seq_along(eigenpairs)) {
      eigenvalue <- eigenpairs[[index]]$eigenvalue
      vector <- eigenpairs[[index]]$vector
      singular_value <- sqrt(eigenvalue * denominator)
      for (position in seq_along(matrix_data$samples)) {
        sample_scores[[position]] <- c(
          sample_scores[[position]],
          vector[[position]] * singular_value
        )
      }
      loadings <- data.frame(
        feature = matrix_data$features,
        loading = vapply(matrix_data$rows, function(row) dot(row, vector) / singular_value, numeric(1)),
        stringsAsFactors = FALSE
      )
      loadings <- loadings[order(-loadings$loading, loadings$feature, method = "radix"), ]
      # The engine takes the first 10 positive loadings of the descending
      # list and the first 10 negative ones of the REVERSED list, so tie
      # order inside both tails must follow that exactly. `rev()` on a
      # data.frame would reverse the columns, hence the explicit row index.
      top_positive <- head(loadings[loadings$loading > 0, ], 10L)
      negatives <- loadings[loadings$loading < 0, ]
      top_negative <- if (nrow(negatives) == 0L) negatives else head(
        negatives[rev(seq_len(nrow(negatives))), ],
        10L
      )
      components[[index]] <- list(
        component = index,
        eigenvalue = eigenvalue,
        explained_variance_percent = min(max(eigenvalue / total_variance * 100, 0), 100),
        top_positive_loadings = named_pair_list(top_positive),
        top_negative_loadings = named_pair_list(top_negative)
      )
    }

    warnings_out <- list()
    if (centered$constant_features != 0L) {
      warnings_out <- c(warnings_out, paste0(
        centered$constant_features, " constant features contributed no PCA variance"
      ))
    }
    if (component_limit < components_requested) {
      warnings_out <- c(warnings_out, paste0(
        "requested ", components_requested,
        " components but matrix rank permits at most ", component_limit
      ))
    }

    list(
      feature_count = length(matrix_data$features),
      sample_count = length(matrix_data$samples),
      scaled_features = scale_features,
      total_variance = total_variance,
      components = components,
      samples = unname(Map(
        function(sample, scores) list(sample = sample, scores = as.list(scores)),
        matrix_data$samples,
        sample_scores
      )),
      warnings = warnings_out
    )
  }
)

`%||%` <- function(left, right) if (is.null(left)) right else left

named_pair_list <- function(frame) {
  # unname: the JSON side must see an ARRAY of {feature, loading} objects,
  # not an object keyed by feature id.
  unname(Map(
    function(feature, loading) list(feature = feature, loading = loading),
    frame$feature,
    frame$loading
  ))
}

count_char <- function(text, character) {
  matches <- gregexpr(character, text, fixed = TRUE)[[1L]]
  if (length(matches) == 1L && matches[[1L]] == -1L) 0L else length(matches)
}

read_pca_matrix <- function(path) {
  magic <- readBin(path, "raw", n = 2L)
  connection <- if (length(magic) == 2L && identical(magic, as.raw(c(0x1f, 0x8b)))) {
    gzfile(path, "rt")
  } else {
    file(path, "rt")
  }
  lines <- tryCatch(readLines(connection, warn = FALSE), finally = close(connection))
  nonblank <- lines[nzchar(trimws(lines))]
  if (length(nonblank) == 0L) implementation_error("expression matrix is empty")
  delimiter <- if (count_char(nonblank[[1L]], "\t") >= count_char(nonblank[[1L]], ",")) "\t" else ","
  split_lines <- strsplit(nonblank, delimiter, fixed = TRUE)
  headers <- trimws(split_lines[[1L]])
  if (length(headers) < 2L || !nzchar(headers[[1L]])) {
    implementation_error("expected a named feature identifier column and at least one sample")
  }
  samples <- headers[-1L]
  if (any(!nzchar(samples)) || anyDuplicated(samples) != 0L) {
    implementation_error("sample names must be non-empty and unique")
  }
  features <- character(0)
  rows <- list()
  seen <- character(0)
  for (row_number in seq_along(split_lines)[-1L]) {
    record <- split_lines[[row_number]]
    if (length(record) != length(headers)) {
      implementation_error(paste0(
        "record ", row_number, " has ", length(record),
        " fields, expected ", length(headers)
      ))
    }
    feature <- trimws(record[[1L]])
    if (!nzchar(feature) || feature %in% seen) {
      implementation_error(paste0("feature identifier ", feature, " is empty or duplicated"))
    }
    seen <- c(seen, feature)
    row <- numeric(length(samples))
    for (position in seq_along(samples)) {
      value <- trimws(record[[position + 1L]])
      if (value == "" || value == "." || tolower(value) %in% c("na", "nan")) {
        implementation_error(paste0(
          "sample ", samples[[position]], " contains a missing value; ",
          "impute or filter it before this analysis"
        ))
      }
      parsed <- suppressWarnings(as.numeric(value))
      if (is.na(parsed)) {
        implementation_error(paste0(
          "sample ", samples[[position]], " contains non-numeric value ", value
        ))
      }
      if (!is.finite(parsed)) {
        implementation_error(paste0("sample ", samples[[position]], " contains a non-finite value"))
      }
      row[[position]] <- parsed
    }
    features <- c(features, feature)
    rows <- c(rows, list(row))
  }
  if (length(rows) == 0L) {
    implementation_error("expression matrix contains no feature rows")
  }
  list(features = features, samples = samples, rows = rows, values = rows)
}

centered_rows <- function(values, scale) {
  constant_features <- 0L
  rows <- vector("list", length(values))
  for (index in seq_along(values)) {
    row <- values[[index]]
    mean <- sum(row) / length(row)
    centered <- row - mean
    sum_squares <- sum(centered * centered)
    if (sum_squares <= .Machine$double.eps) {
      constant_features <- constant_features + 1L
      centered[] <- 0
    } else if (isTRUE(scale)) {
      denominator <- max(length(row) - 1L, 1L)
      centered <- centered / sqrt(sum_squares / denominator)
    }
    rows[[index]] <- centered
  }
  list(rows = rows, constant_features = constant_features)
}

dot <- function(left, right) sum(left * right)

orthogonalize <- function(vector, basis) {
  for (direction in basis) {
    projection <- dot(vector, direction)
    vector <- vector - projection * direction
  }
  vector
}

normalize_vector <- function(vector) {
  norm <- sqrt(dot(vector, vector))
  if (norm > .Machine$double.eps) vector <- vector / norm
  list(vector = vector, norm = norm)
}

gram_multiply <- function(rows, vector, denominator) {
  result <- numeric(length(vector))
  for (row in rows) {
    projection <- dot(row, vector) / denominator
    result <- result + row * projection
  }
  result
}

leading_eigenpairs <- function(rows, component_count, denominator) {
  if (component_count <= 0L) return(list())
  sample_count <- length(rows[[1L]])
  eigenvectors <- list()
  eigenpairs <- list()
  for (component in seq_len(component_count) - 1L) {
    indices <- seq_len(sample_count)
    phase <- indices * (component + 2L)
    vector <- sin(phase) + cos(phase * 0.37)
    vector <- orthogonalize(vector, eigenvectors)
    normalized <- normalize_vector(vector)
    vector <- normalized$vector
    if (normalized$norm <= .Machine$double.eps) {
      found <- FALSE
      for (basis in indices) {
        vector <- rep(0, sample_count)
        vector[[basis]] <- 1
        vector <- orthogonalize(vector, eigenvectors)
        normalized <- normalize_vector(vector)
        vector <- normalized$vector
        if (normalized$norm > .Machine$double.eps) {
          found <- TRUE
          break
        }
      }
      if (!found) break
    }
    for (iteration in 1:500) {
      next_vector <- gram_multiply(rows, vector, denominator)
      next_vector <- orthogonalize(next_vector, eigenvectors)
      normalized <- normalize_vector(next_vector)
      next_vector <- normalized$vector
      if (normalized$norm <= 1e-14) break
      alignment <- abs(dot(vector, next_vector))
      vector <- next_vector
      if (1.0 - alignment < 1e-11) break
    }
    projected <- gram_multiply(rows, vector, denominator)
    eigenvalue <- dot(vector, projected)
    if (!is.finite(eigenvalue) || eigenvalue <= 1e-12) break
    largest_positions <- which(abs(vector) == max(abs(vector)))
    largest <- largest_positions[[length(largest_positions)]]
    if (vector[[largest]] < 0) vector <- -vector
    eigenvectors <- c(eigenvectors, list(vector))
    eigenpairs <- c(eigenpairs, list(list(eigenvalue = eigenvalue, vector = vector)))
  }
  eigenpairs
}
