# Smoke and parity tests for the R benchmark harness (M2-T3).
#
# Run with:  Rscript workflows/org.linxira.benchmark-r/tests/test_benchmark_harness.R
# jsonlite and digest must resolve (LINXIRA_BIO_WORKFLOW_R_LIBRARY or the
# interpreter library); Biostrings-dependent cases are skipped when the
# package is missing so the suite stays runnable on a bare R.

arguments_all <- commandArgs(trailingOnly = FALSE)
test_file <- sub("^--file=", "", grep("^--file=", arguments_all, value = TRUE))
pack_root <- normalizePath(file.path(dirname(test_file), ".."), winslash = "/", mustWork = TRUE)
repository_root <- normalizePath(file.path(pack_root, "..", ".."), winslash = "/", mustWork = TRUE)
tiny_fasta <- file.path(repository_root, "tests", "fixtures", "sequences", "tiny.fa")
stopifnot(file.exists(tiny_fasta))

source(file.path(pack_root, "src", "benchmark_harness.R"))
SCRIPT_DIRECTORY <- file.path(pack_root, "src")
configure_library()
invisible(check_packages(HARNESS_PACKAGES))
have_biostrings <- requireNamespace("Biostrings", quietly = TRUE)

# `linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json`
# (Rust engine 1.0.1); the R implementation must agree within 1e-6 relative.
rust_tiny_result <- list(
  sequence_count = 3, total_bases = 12, min_length = 2, max_length = 6,
  mean_length = 4.0, n50 = 6, l50 = 1, au_n = 4.666666666666667,
  gc_percent = 60.0, n_count = 2, n_percent = 16.666666666666664
)

assert_close <- function(expected, actual, field) {
  scale <- max(abs(expected), abs(actual))
  if (scale == 0) {
    stopifnot(expected == actual)
  } else if (abs(expected - actual) / scale > 1e-6) {
    stop(sprintf("%s: expected %s, got %s", field, format(expected, digits = 17), format(actual, digits = 17)))
  }
}

assert_matches_rust <- function(result) {
  stopifnot(setequal(names(result), names(rust_tiny_result)))
  for (field in names(rust_tiny_result)) assert_close(rust_tiny_result[[field]], result[[field]], field)
}

make_request <- function(fasta, output_directory, capability = "sequence.stats.v1", backend = "r",
                         extra_parameters = list()) {
  list(
    schema_version = "2",
    job_id = "benchmark-sequence-stats-v1",
    capability = capability,
    inputs = list(list(
      artifact_id = "input-fasta",
      role = "fasta",
      cardinality = "single",
      files = list(list(
        file_id = "input-fasta-1",
        path = fasta,
        format = "fasta",
        compression = "none",
        size_bytes = as.numeric(file.info(fasta)$size),
        sha256 = sha256_file(fasta)
      ))
    )),
    execution = list(mode = "local-cpu", backend = backend),
    parameters = c(list(output_directory = output_directory), extra_parameters)
  )
}

run_harness <- function(request, workspace) {
  request_path <- file.path(workspace, "request.json")
  jsonlite::write_json(request, request_path, auto_unbox = TRUE, digits = NA)
  result_path <- file.path(request$parameters$output_directory, "result.json")
  code <- suppressMessages(main(c("--request", request_path, "--result", result_path)))
  envelope <- jsonlite::fromJSON(result_path, simplifyVector = FALSE)
  list(code = code, envelope = envelope)
}

with_workspace <- function(body) {
  workspace <- tempfile("linxira-benchmark-r-test-")
  dir.create(workspace)
  on.exit(unlink(workspace, recursive = TRUE, force = TRUE), add = TRUE)
  body(normalizePath(workspace, winslash = "/", mustWork = TRUE))
}

# --- argument parsing -------------------------------------------------------
stopifnot(inherits(try(parse_arguments(c("--request", "a")), silent = TRUE), "try-error"))
stopifnot(inherits(try(parse_arguments(c("--request", "a", "--request", "b")), silent = TRUE), "try-error"))
parsed <- parse_arguments(c("--result", "r.json", "--request", "q.json"))
stopifnot(identical(parsed$request, "q.json"), identical(parsed$result, "r.json"))

stopifnot(version_satisfies("2.72.1", ">=2.70.0,<3.0.0"))
stopifnot(!version_satisfies("3.0.0", ">=2.70.0,<3.0.0"))

# --- registry ---------------------------------------------------------------
registry <- load_implementations(file.path(SCRIPT_DIRECTORY, "implementations"))
stopifnot("sequence.stats.v1" %in% names(registry))
stopifnot(identical(registry[["sequence.stats.v1"]]$input_roles, "fasta"))
stopifnot(length(registry[["sequence.stats.v1"]]$parameters) == 0L)

# --- validation failures produce error envelopes ----------------------------
with_workspace(function(workspace) {
  outcome <- run_harness(make_request(tiny_fasta, file.path(workspace, "out"), capability = "sequence.orf.v1"), workspace)
  stopifnot(outcome$code == 2L, identical(outcome$envelope$status, "error"))
  stopifnot(identical(outcome$envelope$capability, "sequence.orf.v1"))
  stopifnot(identical(outcome$envelope$job_id, "benchmark-sequence-stats-v1"))
  stopifnot(grepl("no r implementation", outcome$envelope$diagnostics[[1L]]$message))
})
with_workspace(function(workspace) {
  outcome <- run_harness(make_request(tiny_fasta, file.path(workspace, "out"), backend = "python"), workspace)
  stopifnot(outcome$code == 2L, grepl("execution.backend", outcome$envelope$diagnostics[[1L]]$message))
})
with_workspace(function(workspace) {
  request <- make_request(tiny_fasta, file.path(workspace, "out"), extra_parameters = list(min_length = 10))
  outcome <- run_harness(request, workspace)
  stopifnot(outcome$code == 2L, grepl("does not accept parameter min_length", outcome$envelope$diagnostics[[1L]]$message))
})
with_workspace(function(workspace) {
  request <- make_request(tiny_fasta, file.path(workspace, "out"))
  request_path <- file.path(workspace, "request.json")
  jsonlite::write_json(request, request_path, auto_unbox = TRUE, digits = NA)
  stray_directory <- file.path(workspace, "elsewhere")
  dir.create(stray_directory)
  stray <- file.path(stray_directory, "result.json")
  code <- suppressMessages(main(c("--request", request_path, "--result", stray)))
  envelope <- jsonlite::fromJSON(stray, simplifyVector = FALSE)
  stopifnot(code == 2L, grepl("output_directory", envelope$diagnostics[[1L]]$message))
})

if (!have_biostrings) {
  message("Biostrings is not installed; parity cases skipped")
} else {
  implementation <- registry[["sequence.stats.v1"]]

  assert_matches_rust(implementation$run(list(fasta = tiny_fasta), list()))

  with_workspace(function(workspace) {
    compressed <- file.path(workspace, "reads.fa.gz")
    connection <- gzfile(compressed, "wb")
    writeLines(c(">one", "ACGT", ">two", "NN"), connection)
    close(connection)
    result <- implementation$run(list(fasta = compressed), list())
    stopifnot(result$sequence_count == 2, result$total_bases == 6, result$n_count == 2)
    assert_close(50, result$gc_percent, "gc_percent")
  })

  with_workspace(function(workspace) {
    cases <- list(
      "sequence before header" = "ACGT",
      "empty identifier" = c(">", "ACGT"),
      "no records" = ""
    )
    for (name in names(cases)) {
      path <- file.path(workspace, "case.fa")
      writeLines(cases[[name]], path)
      outcome <- try(implementation$run(list(fasta = path), list()), silent = TRUE)
      if (!inherits(outcome, "try-error")) stop(sprintf("%s should fail", name))
    }
  })

  with_workspace(function(workspace) {
    output_directory <- file.path(workspace, "analysis", "sequence.stats.v1_run")
    dir.create(dirname(output_directory), recursive = TRUE)
    outcome <- run_harness(make_request(tiny_fasta, output_directory), workspace)
    stopifnot(outcome$code == 0L)
    envelope <- outcome$envelope
    stopifnot(identical(envelope$status, "ok"), identical(envelope$capability, "sequence.stats.v1"))
    assert_matches_rust(envelope$result)

    stopifnot(length(envelope$artifacts) == 1L)
    artifact <- envelope$artifacts[[1L]]
    stopifnot(same_path(dirname(artifact$path), output_directory))
    stopifnot(file.exists(artifact$path))
    stopifnot(identical(artifact$sha256, sha256_file(artifact$path)))
    stopifnot(artifact$size_bytes == file.info(artifact$path)$size)
    assert_matches_rust(jsonlite::fromJSON(artifact$path, simplifyVector = FALSE))

    provenance <- envelope$provenance
    stopifnot(identical(provenance$input_sha256$fasta, sha256_file(tiny_fasta)))
    stopifnot(identical(provenance$dependency_lock_sha256, sha256_file(file.path(pack_root, "dependencies.lock.json"))))
    stopifnot(any(vapply(provenance$software, function(entry) identical(entry$name, "Biostrings"), logical(1))))

    self_reported <- Filter(function(d) identical(d$code, SELF_REPORTED_CODE), envelope$diagnostics)
    stopifnot(length(self_reported) == 1L, identical(self_reported[[1L]]$severity, "info"))
    metrics <- jsonlite::fromJSON(self_reported[[1L]]$message, simplifyVector = FALSE)
    stopifnot(is.numeric(metrics$wall_ms), metrics$wall_ms >= 0, identical(metrics$backend, "r"))
    if (file.exists("/proc/self/status")) stopifnot(is.numeric(metrics$peak_rss_mb), metrics$peak_rss_mb > 0)
  })

  with_workspace(function(workspace) {
    output_directory <- file.path(workspace, "out")
    dir.create(output_directory)
    outcome <- run_harness(make_request(tiny_fasta, output_directory), workspace)
    stopifnot(outcome$code == 2L, grepl("output directory appeared", outcome$envelope$diagnostics[[1L]]$message))
  })
}

# --- M3 #5/#7: PCA and Venn parity (no Biostrings needed) -------------------
reference_dir <- file.path(pack_root, "tests", "reference")
read_reference <- function(name) {
  jsonlite::fromJSON(file.path(reference_dir, name), simplifyVector = FALSE)
}
roundtrip <- function(value) {
  jsonlite::fromJSON(
    jsonlite::toJSON(value, auto_unbox = TRUE, null = "null", na = "null", digits = NA),
    simplifyVector = FALSE
  )
}
parity_max_rel <- 0
parity_compare <- function(path, expected, got) {
  if (is.null(expected) && is.null(got)) return(invisible())
  if (is.list(expected) && !is.null(names(expected))) {
    stopifnot(setequal(names(expected), names(got)))
    for (name in names(expected)) {
      parity_compare(paste0(path, "$", name), expected[[name]], got[[name]])
    }
  } else if (is.list(expected)) {
    stopifnot(length(expected) == length(got))
    for (index in seq_along(expected)) {
      parity_compare(paste0(path, "[", index, "]"), expected[[index]], got[[index]])
    }
  } else if (is.character(expected)) {
    if (!identical(as.character(expected), as.character(got))) {
      stop(sprintf("%s: %s vs %s", path,
                   paste(expected, collapse = ","), paste(got, collapse = ",")))
    }
  } else if (is.logical(expected)) {
    stopifnot(identical(isTRUE(expected), isTRUE(got)))
  } else if (is.numeric(expected)) {
    scale <- max(abs(expected), abs(got), na.rm = TRUE)
    if (scale == 0) {
      stopifnot(expected == got)
    } else {
      relative <- abs(expected - got) / scale
      if (relative > 1e-6) {
        stop(sprintf("%s: %s vs %s (relative %g)", path,
                     format(expected, digits = 17), format(got, digits = 17), relative))
      }
      parity_max_rel <<- max(parity_max_rel, relative)
    }
  } else {
    stop(paste0(path, ": unhandled reference type ", class(expected)))
  }
  invisible()
}

sets_table <- file.path(repository_root, "tests", "fixtures", "set-analysis", "sets.tsv")
pca_matrix <- file.path(repository_root, "tests", "fixtures", "expression-matrix", "deseq2-counts.csv")

with_workspace(function(workspace) {
  registry <- load_implementations(file.path(SCRIPT_DIRECTORY, "implementations"))
  venn <- registry[["set.venn.v1"]]
  pca <- registry[["expression.pca.v1"]]
  stopifnot(!is.null(venn), !is.null(pca))

  parity_compare("$", read_reference("set.venn.v1.default.json"),
                 roundtrip(venn$run(list(table = sets_table), list())))
  parity_compare("$", read_reference("set.venn.v1.items.json"),
                 roundtrip(venn$run(list(table = sets_table), list(include_items = TRUE))))

  wide <- file.path(workspace, "wide.tsv")
  writeLines(paste(paste0("set", 0:6), collapse = "\t"), wide)
  outcome <- try(venn$run(list(table = wide), list()), silent = TRUE)
  stopifnot(inherits(outcome, "try-error"))

  parity_compare("$", read_reference("expression.pca.v1.k3.json"),
                 roundtrip(pca$run(list(matrix = pca_matrix), list(components = 3))))

  # --- M3 #3: PDB summary parity (four recorded engine outputs) ------------
  pdb_impl <- registry[["structure.pdb.summary.v1"]]
  stopifnot(!is.null(pdb_impl))
  pdb_cases <- list(
    list("structure.pdb.summary.v1.default.json",
         file.path(repository_root, "tests", "fixtures", "structure-pdb-summary", "alphafold-style.pdb"),
         FALSE),
    list("structure.pdb.summary.v1.plddt.json",
         file.path(repository_root, "tests", "fixtures", "structure-pdb-summary", "alphafold-style.pdb"),
         TRUE),
    list("structure.pdb.summary.v1.reference-default.json",
         file.path(repository_root, "tests", "fixtures", "structure-analysis", "reference.pdb"),
         FALSE),
    list("structure.pdb.summary.v1.reference-plddt.json",
         file.path(repository_root, "tests", "fixtures", "structure-analysis", "reference.pdb"),
         TRUE)
  )
  for (case in pdb_cases) {
    parity_compare("$", read_reference(case[[1L]]),
                   roundtrip(pdb_impl$run(
                     list(pdb = case[[2L]]),
                     list(interpret_b_factors_as_plddt = case[[3L]])
                   )))
  }
  # Out-of-range B-factor under pLDDT interpretation must fail like the engine.
  bad_plddt <- file.path(workspace, "bad-plddt.pdb")
  writeLines(c(
    "ATOM      1  N   GLY A   1      11.104  13.207   9.657  1.00 99.99           N  ",
    "ATOM      2  CA  GLY A   1      12.204  13.707   9.157  1.00 -0.01           C  "
  ), bad_plddt)
  outcome <- try(pdb_impl$run(
    list(pdb = bad_plddt),
    list(interpret_b_factors_as_plddt = TRUE)
  ), silent = TRUE)
  stopifnot(inherits(outcome, "try-error"))

  bad_missing <- file.path(workspace, "missing.csv")
  writeLines(c("gene,s1,s2", "g1,1,NA", "g2,2,3"), bad_missing)
  outcome <- try(pca$run(list(matrix = bad_missing), list()), silent = TRUE)
  stopifnot(inherits(outcome, "try-error"))

  cat("PCA/Venn/PDB parity OK; max relative error:", format(parity_max_rel), "\n")
})

cat("benchmark-r harness tests passed", if (have_biostrings) "(with Biostrings parity)" else "(validation only)", "\n")
