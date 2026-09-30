# sequence.ssr.v1 — independent Biostrings implementation.
#
# Backend counterpart of the Rust engine's SSR scanner
# (engine/crates/linxira-bio-core/src/ssr.rs) and the pytrf twin in
# org.linxira.benchmark-python. The heavy matching is delegated to
# Biostrings::matchPattern (C kernel); this file only links the fixed-width
# hits into tandem runs and then applies the exact MISA-aligned semantics of
# the engine so all three backends diff field for field:
#
# * shortest-motif-first: at a given position the 1..6-mer whose tandem
#   repeat meets the per-length threshold wins
# * consumed intervals: records never overlap
# * perfect repeats only; fixed=TRUE matching means anything outside ACGT
#   (for example N) never joins a motif run; input is upper-cased first
# * compound grouping: consecutive records within `compound_max_distance` bp
#   form one compound SSR group
#
# Motif enumeration is bounded by oligonucleotideFrequency: only k-mers that
# actually occur in the sequence are matched, not all 4^p possibilities.

IMPLEMENTATION <- list(
  capability = "sequence.ssr.v1",
  input_roles = c("fasta"),
  parameters = c("min_repeats", "compound_max_distance"),
  packages = c(Biostrings = ">=2.70.0,<3.0.0"),
  software = function() {
    list(list(
      name = "Biostrings",
      version = as.character(utils::packageVersion("Biostrings")),
      package_id = "bioconductor-biostrings"
    ))
  },
  run = function(inputs, parameters) {
    path <- inputs[["fasta"]]
    if (!file.exists(path)) {
      implementation_error(sprintf("input file does not exist: %s", path))
    }
    defaults <- c("1" = 10L, "2" = 6L, "3" = 5L, "4" = 5L, "5" = 5L, "6" = 5L)
    thresholds <- defaults
    spec <- parameters[["min_repeats"]]
    if (!is.null(spec)) {
      if (!is.character(spec) || !length(spec) == 1L) {
        implementation_error("min_repeats must be a string like '1:10,2:6'")
      }
      for (entry in strsplit(spec, ",", fixed = TRUE)[[1]]) {
        entry <- trimws(entry)
        if (!nzchar(entry)) next
        parts <- strsplit(entry, ":", fixed = TRUE)[[1]]
        if (length(parts) != 2L) {
          implementation_error(sprintf(
            "min_repeats entries must be <motif-length>:<count>, got '%s'", entry
          ))
        }
        motif_length <- suppressWarnings(as.integer(trimws(parts[[1]])))
        repeats <- suppressWarnings(as.integer(trimws(parts[[2]])))
        if (is.na(motif_length) || is.na(repeats) ||
          motif_length < 1L || motif_length > 6L) {
          implementation_error("motif length must be between 1 and 6")
        }
        if (repeats < 2L) {
          implementation_error("repeat counts must be at least 2")
        }
        thresholds[[as.character(motif_length)]] <- repeats
      }
    }
    compound_distance <- 100
    raw_distance <- parameters[["compound_max_distance"]]
    if (!is.null(raw_distance)) {
      compound_distance <- suppressWarnings(as.integer(raw_distance))
      if (is.na(compound_distance) || compound_distance < 0L) {
        implementation_error("compound_max_distance must be a non-negative integer")
      }
    }

    raw <- Biostrings::readBStringSet(path, format = "fasta")
    if (length(raw) == 0L) {
      implementation_error("FASTA contains no sequence records")
    }
    headers <- names(raw)
    if (is.null(headers) || any(!nzchar(trimws(headers)))) {
      implementation_error("FASTA header has no identifier")
    }
    identifiers <- vapply(
      strsplit(headers, "[[:space:]]+"), function(fields) fields[[1L]], character(1)
    )
    sequences <- Biostrings::DNAStringSet(toupper(as.character(raw)))

    records <- list()
    compound_groups <- 0L
    motif_length_counts <- new.env(parent = emptyenv())
    sequences_summary <- list()
    total_bases <- 0

    for (index in seq_along(sequences)) {
      subject <- sequences[[index]]
      identifier <- identifiers[[index]]
      sequence_length <- Biostrings::width(subject)
      total_bases <- total_bases + sequence_length

      candidates <- NULL
      for (motif_length in 1:6) {
        frequency <- Biostrings::oligonucleotideFrequency(subject, width = motif_length)
        occurring <- names(frequency)[frequency > 0]
        threshold <- thresholds[[as.character(motif_length)]]
        for (motif in occurring) {
          hits <- start(Biostrings::matchPattern(motif, subject, fixed = TRUE))
          if (length(hits) < threshold) next
          # Link consecutive hits spaced exactly one motif apart into runs.
          run_start <- 1L
          for (position in seq_along(hits)) {
            last <- position == length(hits) ||
              hits[[position + 1L]] != hits[[position]] + motif_length
            if (!last) next
            run_length <- position - run_start + 1L
            if (run_length >= threshold) {
              candidates <- rbind(candidates, data.frame(
                start = hits[[run_start]],
                end = hits[[position]] + motif_length - 1L,
                repeats = run_length,
                motif_length = motif_length,
                motif = motif,
                stringsAsFactors = FALSE
              ))
            }
            run_start <- position + 1L
          }
        }
      }

      found <- list()
      if (!is.null(candidates)) {
        candidates <- candidates[order(candidates$start, candidates$motif_length), ]
        consumed_until <- 0L
        for (row in seq_len(nrow(candidates))) {
          candidate <- candidates[row, ]
          if (candidate$start <= consumed_until) next
          found[[length(found) + 1L]] <- list(
            sequence_id = identifier,
            start = as.numeric(candidate$start),
            end = as.numeric(candidate$end),
            motif_length = as.integer(candidate$motif_length),
            motif = candidate$motif,
            repeats = as.integer(candidate$repeats),
            size = as.numeric(candidate$end - candidate$start + 1L),
            compound = FALSE
          )
          consumed_until <- candidate$end
        }
      }

      # Compound grouping: consecutive records within the interruption
      # distance form one compound SSR group.
      run_head <- 1L
      run_length <- 1L
      mark_group <- function(from, to) {
        for (slot in from:to) found[[slot]]$compound <<- TRUE
        compound_groups <<- compound_groups + 1L
      }
      if (length(found) > 1L) {
        for (position in 2:length(found)) {
          gap <- found[[position]]$start - found[[position - 1L]]$end - 1
          if (gap <= compound_distance) {
            run_length <- run_length + 1L
          } else {
            if (run_length > 1L) mark_group(run_head, run_head + run_length - 1L)
            run_head <- position
            run_length <- 1L
          }
        }
      }
      if (run_length > 1L && length(found) > 1L) {
        mark_group(run_head, run_head + run_length - 1L)
      }

      for (record in found) {
        key <- as.character(record$motif_length)
        assign(key, ifelse(is.null(motif_length_counts[[key]]), 0L,
          motif_length_counts[[key]]
        ) + 1L, envir = motif_length_counts)
        records[[length(records) + 1L]] <- record
      }
      sequences_summary[[length(sequences_summary) + 1L]] <- list(
        sequence_id = identifier,
        length = as.numeric(sequence_length),
        ssr_count = length(found)
      )
    }

    counts <- list()
    for (key in sort(as.integer(ls(motif_length_counts)))) {
      counts[[as.character(key)]] <- motif_length_counts[[as.character(key)]]
    }

    list(
      records = records,
      summary = list(
        sequence_count = length(sequences_summary),
        total_bases = total_bases,
        ssr_count = length(records),
        compound_group_count = compound_groups,
        motif_length_counts = counts,
        sequences = sequences_summary
      )
    )
  }
)
