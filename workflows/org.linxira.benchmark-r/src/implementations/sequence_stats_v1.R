# sequence.stats.v1 — independent Biostrings implementation.
#
# Field-for-field counterpart of the Rust engine's `fasta_stats`
# (engine/crates/linxira-bio-core/src/sequence.rs); see the Python twin in
# org.linxira.benchmark-python for the prose definitions. Parsing is done by
# Biostrings::readBStringSet (BString, so protein, RNA and IUPAC letters are all
# accepted like the engine does; gzip is detected by Biostrings itself). The
# engine's error cases are reproduced: no records, header without identifier,
# and sequence text before the first header (Biostrings raises on the last one).

IMPLEMENTATION <- list(
  capability = "sequence.stats.v1",
  input_roles = c("fasta"),
  # No tunables: the contract of sequence.stats.v1 is fixed.
  parameters = character(0),
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
    sequences <- Biostrings::readBStringSet(path, format = "fasta")
    if (length(sequences) == 0L) {
      implementation_error("FASTA contains no records")
    }
    headers <- names(sequences)
    if (is.null(headers) || any(!nzchar(trimws(headers)))) {
      implementation_error("FASTA header has no identifier")
    }
    # Widths as doubles: multi-terabyte inputs overflow R's 32-bit integers.
    widths <- as.numeric(Biostrings::width(sequences))
    # Each element of `letters` is a set counted together, both cases at once.
    counts <- Biostrings::letterFrequency(sequences, letters = c("GCgc", "ATUatu", "Nn"))
    gc_bases <- sum(as.numeric(counts[, 1L]))
    canonical_bases <- gc_bases + sum(as.numeric(counts[, 2L]))
    n_count <- sum(as.numeric(counts[, 3L]))

    lengths_desc <- sort(widths, decreasing = TRUE)
    total_bases <- sum(lengths_desc)
    threshold <- floor((total_bases + 1) / 2)
    n50 <- 0
    l50 <- 0
    if (threshold > 0) {
      cumulative <- cumsum(lengths_desc)
      index <- which(cumulative >= threshold)[[1L]]
      n50 <- lengths_desc[[index]]
      l50 <- index
    }
    au_n <- if (total_bases > 0) sum(lengths_desc * lengths_desc) / total_bases else 0

    list(
      sequence_count = length(sequences),
      total_bases = total_bases,
      min_length = lengths_desc[[length(lengths_desc)]],
      max_length = lengths_desc[[1L]],
      mean_length = ratio(total_bases, length(sequences)),
      n50 = n50,
      l50 = l50,
      au_n = au_n,
      gc_percent = ratio(gc_bases, canonical_bases) * 100,
      n_count = n_count,
      n_percent = ratio(n_count, total_bases) * 100
    )
  }
)

ratio <- function(numerator, denominator) {
  if (denominator == 0) 0 else numerator / denominator
}
