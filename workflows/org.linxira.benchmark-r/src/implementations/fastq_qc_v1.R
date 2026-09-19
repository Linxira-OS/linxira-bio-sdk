# fastq.qc.v1 — independent base-R implementation (exact counter arithmetic).
#
# Field-for-field counterpart of the Rust engine's FASTQ quality-control scan
# (engine/crates/linxira-bio-core/src/fastq.rs); see the Python twin in
# org.linxira.benchmark-python for the prose definitions. Parsing reproduces
# the engine's line-level rules: CRLF tolerance, '@' header with a first
# whitespace-delimited identifier, sequence lines until a '+' separator
# (optionally repeating the identifier), quality lines accumulating to the
# sequence length with bytes in 33..=126, and the engine's error cases
# (no records, truncated record, truncated quality, length mismatch,
# explicit Phred+64 byte below the offset). Counting tracks GC/N
# case-insensitively per cycle (capped at max_cycles) plus file-level
# Phred+33 and Phred+64 Q20/Q30 counters; auto-detection applies Phred+33
# when the minimum quality byte is below 59 and otherwise reports
# "ambiguous" with the engine's warning. No package beyond base R.
#
# The reader and the accumulator are environments (reference semantics) so
# the helpers mutate the same counters, mirroring the engine's single
# QcAccumulator; R's value semantics would silently fork the state.

IMPLEMENTATION <- list(
  capability = "fastq.qc.v1",
  input_roles = c("fastq"),
  # The reader opens through gzfile (magic-byte dispatch); declared gzip
  # inputs are part of the contract.
  input_compression = list(fastq = c("none", "gzip")),
  parameters = c("max_cycles", "quality_encoding"),
  packages = character(0),
  software = function() list(),
  run = function(inputs, parameters) {
    path <- inputs[["fastq"]]
    max_cycles <- fastq_qc_as_count(parameters$max_cycles %||% 500L)
    encoding <- parameters$quality_encoding %||% "auto"
    if (!encoding %in% c("auto", "phred+33", "phred+64")) {
      implementation_error(paste0("unsupported quality encoding: ", encoding))
    }

    lines <- fastq_qc_new_reader(path)
    on.exit(close(lines$connection), add = TRUE)
    state <- fastq_qc_new_state()

    repeat {
      header_line <- fastq_qc_next_line(lines)
      if (is.null(header_line)) break
      record <- state$read_count + 1L
      identifier <- fastq_qc_parse_header(header_line, record, lines$line_number)
      sequence_length <- fastq_qc_parse_sequence(
        lines, identifier, record, state, max_cycles
      )
      fastq_qc_parse_quality(
        lines, record, sequence_length, state, encoding, max_cycles
      )
      state$read_count <- state$read_count + 1L
      state$total_bases <- state$total_bases + sequence_length
      if (is.null(state$min_length) || sequence_length < state$min_length) {
        state$min_length <- sequence_length
      }
      if (sequence_length > state$max_length) state$max_length <- sequence_length
      if (sequence_length > max_cycles) state$cycles_truncated <- TRUE
    }

    if (state$read_count == 0L) implementation_error("FASTQ contains no records")

    if (encoding == "phred+33") {
      quality_encoding <- "phred+33"; offset <- 33L
    } else if (encoding == "phred+64") {
      quality_encoding <- "phred+64"; offset <- 64L
    } else if (state$minimum_quality < 59) {
      quality_encoding <- "phred+33"; offset <- 33L
    } else {
      quality_encoding <- "ambiguous"; offset <- 33L
    }

    warnings_out <- character(0)
    if (quality_encoding == "ambiguous") {
      warnings_out <- c(
        warnings_out,
        paste(
          "quality bytes are compatible with both Phred+33 and legacy Phred+64/Solexa",
          "encodings; metrics use Phred+33 unless quality_encoding is explicitly overridden"
        )
      )
    }
    if (state$cycles_truncated) {
      warnings_out <- c(
        warnings_out,
        sprintf("per-cycle metrics are capped at %d cycle(s)", max_cycles)
      )
    }

    q20_count <- if (offset == 64L) state$q20_64 else state$q20_33
    q30_count <- if (offset == 64L) state$q30_64 else state$q30_33

    list(
      read_count = as.numeric(state$read_count),
      total_bases = as.numeric(state$total_bases),
      min_length = as.numeric(state$min_length),
      max_length = as.numeric(state$max_length),
      mean_length = fastq_qc_ratio(state$total_bases, state$read_count),
      gc_percent = fastq_qc_percent(state$gc_count, state$total_bases),
      n_percent = fastq_qc_percent(state$n_count, state$total_bases),
      mean_quality = fastq_qc_mean_quality(state$quality_sum, state$total_bases, offset),
      q20_percent = fastq_qc_percent(q20_count, state$total_bases),
      q30_percent = fastq_qc_percent(q30_count, state$total_bases),
      quality_encoding = quality_encoding,
      applied_quality_offset = offset,
      per_cycle = unname(Map(
        function(index, cycle) fastq_qc_cycle_metrics(index, cycle, offset),
        seq_along(state$cycles),
        state$cycles
      )),
      warnings = as.list(warnings_out)
    )
  }
)

`%||%` <- function(left, right) if (is.null(left)) right else left

fastq_qc_as_count <- function(value) {
  if (is.null(value) || !is.finite(value) || value < 0) {
    implementation_error("max_cycles must be a non-negative integer")
  }
  count <- suppressWarnings(as.integer(value))
  if (is.na(count)) implementation_error("max_cycles must be a non-negative integer")
  count
}

# gzfile reads both gzip members and plain bytes, like the engine's
# magic-byte dispatch (gzfile(path, "rb") self-opens); readLines splits on
# \n, so one trailing \r is dropped by hand (CRLF tolerance). The reader is
# an environment because the line number must advance across helper calls.
fastq_qc_new_reader <- function(path) {
  reader <- new.env(parent = emptyenv())
  reader$connection <- gzfile(path, "rb")
  reader$line_number <- 0L
  reader
}

fastq_qc_next_line <- function(reader) {
  line <- readLines(reader$connection, n = 1L, warn = FALSE)
  if (length(line) == 0L) return(NULL)
  reader$line_number <- reader$line_number + 1L
  sub("\r$", "", line)
}

fastq_qc_bytes <- function(line) {
  if (!nzchar(line)) return(integer(0))
  as.integer(charToRaw(line))
}

fastq_qc_first_field <- function(bytes) {
  whitespace <- c(32L, 9L, 10L, 13L, 11L, 12L)
  start <- 1L
  while (start <= length(bytes) && bytes[[start]] %in% whitespace) start <- start + 1L
  end <- length(bytes)
  while (end >= start && bytes[[end]] %in% whitespace) end <- end - 1L
  if (end < start) return(NULL)
  slice <- bytes[start:end]
  cut <- which(slice %in% whitespace)
  if (length(cut) > 0L) slice <- slice[seq_len(cut[[1L]] - 1L)]
  if (length(slice) == 0L) return(NULL)
  rawToChar(as.raw(slice))
}

fastq_qc_parse_header <- function(line, record, line_number) {
  bytes <- fastq_qc_bytes(line)
  if (length(bytes) == 0L || bytes[[1L]] != as.integer(charToRaw("@"))) {
    implementation_error(sprintf(
      "malformed FASTQ record %d at line %d: expected a header beginning with '@'",
      record, line_number
    ))
  }
  identifier <- fastq_qc_first_field(bytes[-1L])
  if (is.null(identifier)) {
    implementation_error(sprintf(
      "malformed FASTQ record %d at line %d: header has no identifier",
      record, line_number
    ))
  }
  identifier
}

fastq_qc_new_state <- function() {
  state <- new.env(parent = emptyenv())
  state$read_count <- 0L
  state$total_bases <- 0
  state$min_length <- NULL
  state$max_length <- 0
  state$gc_count <- 0
  state$n_count <- 0
  state$quality_sum <- 0
  state$minimum_quality <- 256L
  state$q20_33 <- 0L
  state$q30_33 <- 0L
  state$q20_64 <- 0L
  state$q30_64 <- 0L
  state$cycles <- list()
  state$cycles_truncated <- FALSE
  state
}

fastq_qc_cycle_slot <- function(state, index) {
  while (length(state$cycles) < index) {
    state$cycles[[length(state$cycles) + 1L]] <- list(
      base_count = 0, gc_count = 0, n_count = 0, quality_sum = 0,
      q20_33 = 0L, q30_33 = 0L, q20_64 = 0L, q30_64 = 0L
    )
  }
  state$cycles[[index]]
}

fastq_qc_add_base <- function(state, cycle_index, byte, max_cycles) {
  upper <- byte
  if (byte == 0x67 || byte == 0x63 || byte == 0x6E) upper <- byte - 0x20  # g c n
  is_gc <- upper == 0x47 || upper == 0x43  # G C
  is_n <- upper == 0x4E                    # N
  if (is_gc) state$gc_count <- state$gc_count + 1
  if (is_n) state$n_count <- state$n_count + 1
  if (cycle_index <= max_cycles) {
    slot <- fastq_qc_cycle_slot(state, cycle_index)
    slot$base_count <- slot$base_count + 1
    if (is_gc) slot$gc_count <- slot$gc_count + 1
    if (is_n) slot$n_count <- slot$n_count + 1
    state$cycles[[cycle_index]] <- slot
  }
  invisible(NULL)
}

fastq_qc_add_quality <- function(state, cycle_index, byte, max_cycles) {
  state$quality_sum <- state$quality_sum + byte
  if (byte < state$minimum_quality) state$minimum_quality <- byte
  if (byte >= 53L) state$q20_33 <- state$q20_33 + 1L
  if (byte >= 63L) state$q30_33 <- state$q30_33 + 1L
  if (byte >= 84L) state$q20_64 <- state$q20_64 + 1L
  if (byte >= 94L) state$q30_64 <- state$q30_64 + 1L
  if (cycle_index <= max_cycles) {
    slot <- fastq_qc_cycle_slot(state, cycle_index)
    slot$quality_sum <- slot$quality_sum + byte
    if (byte >= 53L) slot$q20_33 <- slot$q20_33 + 1L
    if (byte >= 63L) slot$q30_33 <- slot$q30_33 + 1L
    if (byte >= 84L) slot$q20_64 <- slot$q20_64 + 1L
    if (byte >= 94L) slot$q30_64 <- slot$q30_64 + 1L
    state$cycles[[cycle_index]] <- slot
  }
  invisible(NULL)
}

fastq_qc_parse_sequence <- function(lines, identifier, record, state, max_cycles) {
  sequence_length <- 0
  repeat {
    line <- fastq_qc_next_line(lines)
    if (is.null(line)) {
      implementation_error(sprintf(
        "truncated FASTQ record %d at line %d: expected a '+' separator line",
        record, lines$line_number + 1L
      ))
    }
    bytes <- fastq_qc_bytes(line)
    if (length(bytes) > 0L && bytes[[1L]] == as.integer(charToRaw("+"))) {
      if (sequence_length == 0) {
        implementation_error(sprintf(
          "malformed FASTQ record %d at line %d: sequence is empty",
          record, lines$line_number
        ))
      }
      separator_identifier <- fastq_qc_first_field(bytes[-1L])
      if (!is.null(separator_identifier) && !identical(separator_identifier, identifier)) {
        implementation_error(sprintf(
          "malformed FASTQ record %d at line %d: separator identifier does not match the header identifier",
          record, lines$line_number
        ))
      }
      return(sequence_length)
    }
    if (length(bytes) == 0L) {
      implementation_error(sprintf(
        "malformed FASTQ record %d at line %d: sequence line is empty",
        record, lines$line_number
      ))
    }
    for (column in seq_along(bytes)) {
      byte <- bytes[[column]]
      if (byte <= 0x20 || byte >= 0x7F) {
        implementation_error(sprintf(
          "malformed FASTQ record %d at line %d: invalid sequence byte 0x%02x at column %d",
          record, lines$line_number, byte, column
        ))
      }
      fastq_qc_add_base(state, sequence_length + 1L, byte, max_cycles)
      sequence_length <- sequence_length + 1
    }
  }
}

fastq_qc_parse_quality <- function(lines, record, sequence_length, state, encoding, max_cycles) {
  quality_length <- 0
  while (quality_length < sequence_length) {
    line <- fastq_qc_next_line(lines)
    if (is.null(line)) {
      implementation_error(sprintf(
        "truncated FASTQ record %d at line %d: sequence length is %d, but only %d quality values were present",
        record, lines$line_number + 1L, sequence_length, quality_length
      ))
    }
    bytes <- fastq_qc_bytes(line)
    if (length(bytes) == 0L) {
      implementation_error(sprintf(
        "malformed FASTQ record %d at line %d: quality line is empty",
        record, lines$line_number
      ))
    }
    resulting_length <- quality_length + length(bytes)
    if (resulting_length > sequence_length) {
      implementation_error(sprintf(
        "malformed FASTQ record %d at line %d: sequence length is %d, but quality length is %d",
        record, lines$line_number, sequence_length, resulting_length
      ))
    }
    for (column in seq_along(bytes)) {
      byte <- bytes[[column]]
      if (byte < 33L || byte > 126L) {
        implementation_error(sprintf(
          "malformed FASTQ record %d at line %d: invalid quality byte 0x%02x at column %d",
          record, lines$line_number, byte, column
        ))
      }
      if (encoding == "phred+64" && byte < 64L) {
        implementation_error(sprintf(
          "malformed FASTQ record %d at line %d: quality byte 0x%02x at column %d is below the Phred+64 offset",
          record, lines$line_number, byte, column
        ))
      }
      fastq_qc_add_quality(state, quality_length + column, byte, max_cycles)
    }
    quality_length <- resulting_length
  }
  invisible(NULL)
}

fastq_qc_cycle_metrics <- function(index, cycle, quality_offset) {
  q20_count <- if (quality_offset == 64L) cycle$q20_64 else cycle$q20_33
  q30_count <- if (quality_offset == 64L) cycle$q30_64 else cycle$q30_33
  list(
    cycle = index,
    base_count = as.numeric(cycle$base_count),
    gc_percent = fastq_qc_percent(cycle$gc_count, cycle$base_count),
    n_percent = fastq_qc_percent(cycle$n_count, cycle$base_count),
    mean_quality = fastq_qc_mean_quality(cycle$quality_sum, cycle$base_count, quality_offset),
    q20_percent = fastq_qc_percent(q20_count, cycle$base_count),
    q30_percent = fastq_qc_percent(q30_count, cycle$base_count)
  )
}

fastq_qc_ratio <- function(numerator, denominator) {
  if (denominator == 0) 0 else numerator / denominator
}

fastq_qc_percent <- function(numerator, denominator) {
  fastq_qc_ratio(numerator, denominator) * 100
}

fastq_qc_mean_quality <- function(quality_sum, base_count, quality_offset) {
  if (base_count == 0) 0 else quality_sum / base_count - quality_offset
}
