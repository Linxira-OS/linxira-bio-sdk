# variant.stats.v1 — independent base-R implementation.
#
# Field-for-field counterpart of the Rust engine's vcf_stats
# (engine/crates/linxira-bio-core/src/variant.rs); the benchmark harness
# diffs the result object of both backends with a 1e-6 relative tolerance.
# Semantics mirrored line by line:
#   * first line must be ##fileformat=VCFv<major>.<minor> (ASCII digits)
#   * ##key=value meta lines: key matches ^[A-Za-z0-9._-]+$
#   * #CHROM header: the 8 fixed columns, optional FORMAT column 9 and
#     unique non-empty sample names; records must match the column count
#   * ALT "." means no alleles; empty/"." entries inside a list malformed
#   * classes: symbolic (*, <DEL>, containing [ or ]), indel (unequal
#     length), SNP (length 1), MNV (equal length >= 2); transitions only
#     for single-allele SNP records with A/C/G/T first bases
#   * GT must be the first FORMAT field; alleles split on / and |;
#     "." marks a missing genotype; indices above the ALT count malformed
#   * gzip detected by magic bytes (matches vcf_stats_path)

CAPABILITY <- "variant.stats.v1"
INPUT_ROLES <- "vcf"
INPUT_COMPRESSION <- list(vcf = c("none", "gzip"))
PARAMETERS <- character(0)

NO_SAMPLES_WARNING <- "VCF header declares no samples; genotype metrics are unavailable"
NO_RECORDS_WARNING <- "VCF contains no variant records"

variant_invalid_header <- function(line, message) {
  implementation_error(sprintf("invalid VCF header at line %d: %s", line, message))
}

variant_malformed <- function(line, message) {
  implementation_error(sprintf("malformed VCF record at line %d: %s", line, message))
}

variant_validate_file_format <- function(line) {
  prefix <- "##fileformat=VCFv"
  if (!startsWith(line, prefix)) {
    variant_invalid_header(1, "the first line must be a ##fileformat=VCFv... declaration")
  }
  version <- substring(line, nchar(prefix) + 1L)
  components <- strsplit(version, ".", fixed = TRUE)[[1L]]
  if (length(components) != 2L ||
      !all(nzchar(components)) ||
      !all(grepl("^[0-9]+$", components))) {
    variant_invalid_header(1, sprintf("invalid VCF version '%s'", version))
  }
  invisible(NULL)
}

variant_validate_meta_line <- function(line, line_number) {
  body <- substring(line, 3L)
  parts <- strsplit(body, "=", fixed = TRUE)[[1L]]
  if (length(parts) < 2L) {
    variant_invalid_header(line_number, "meta-information lines must use ##key=value syntax")
  }
  key <- parts[[1L]]
  if (!nzchar(key) || !grepl("^[A-Za-z0-9._-]+$", key)) {
    variant_invalid_header(line_number, sprintf("invalid meta-information key '%s'", key))
  }
  invisible(NULL)
}

variant_parse_column_header <- function(line, line_number) {
  required <- c("#CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO")
  columns <- strsplit(line, "\t", fixed = TRUE)[[1L]]
  if (length(columns) < length(required)) {
    variant_invalid_header(line_number, sprintf(
      "expected at least 8 tab-separated columns, found %d", length(columns)))
  }
  for (index in seq_along(required)) {
    if (!identical(columns[[index]], required[[index]])) {
      variant_invalid_header(line_number, sprintf(
        "column %d must be %s, found '%s'", index, required[[index]], columns[[index]]))
    }
  }
  if (length(columns) > length(required) && !identical(columns[[9L]], "FORMAT")) {
    variant_invalid_header(line_number, sprintf(
      "column 9 must be FORMAT, found '%s'", columns[[9L]]))
  }
  samples <- if (length(columns) > 9L) columns[-(1:9)] else character(0)
  seen <- character(0)
  for (sample in samples) {
    if (!nzchar(sample)) {
      variant_invalid_header(line_number, "sample names must not be empty")
    }
    if (sample %in% seen) {
      variant_invalid_header(line_number, sprintf("duplicate sample name '%s'", sample))
    }
    seen <- c(seen, sample)
  }
  list(column_count = length(columns), sample_count = length(samples))
}

variant_parse_alternates <- function(field, line_number) {
  if (identical(field, ".")) return(character(0))
  alleles <- strsplit(field, ",", fixed = TRUE)[[1L]]
  if (any(!nzchar(alleles)) || any(alleles == ".")) {
    variant_malformed(line_number,
      "ALT contains an empty or missing allele in a non-missing allele list")
  }
  alleles
}

variant_classify_allele <- function(reference, alternate) {
  if (identical(alternate, "*") ||
      (startsWith(alternate, "<") && endsWith(alternate, ">")) ||
      grepl("[", alternate, fixed = TRUE) || grepl("]", alternate, fixed = TRUE)) {
    "symbolic"
  } else if (nchar(reference) != nchar(alternate)) {
    "indel"
  } else if (nchar(reference) == 1L) {
    "snp"
  } else {
    "mnv"
  }
}

variant_substitution_kind <- function(reference, alternate) {
  if (!nzchar(reference) || !nzchar(alternate)) return(NA_character_)
  ref <- toupper(substring(reference, 1L, 1L))
  alt <- toupper(substring(alternate, 1L, 1L))
  if (!ref %in% c("A", "C", "G", "T") || !alt %in% c("A", "C", "G", "T") || ref == alt) {
    return(NA_character_)
  }
  transitions <- c("AG", "GA", "CT", "TC")
  if (paste0(ref, alt) %in% transitions) "transition" else "transversion"
}

variant_genotype_summary <- function(genotype, alternate_count, line_number) {
  if (!nzchar(genotype)) {
    variant_malformed(line_number, "GT value is empty")
  }
  missing <- FALSE
  alternate_alleles <- 0
  for (allele in strsplit(gsub("/", "|", genotype, fixed = TRUE), "|", fixed = TRUE)[[1L]]) {
    if (!nzchar(allele)) {
      variant_malformed(line_number, sprintf("invalid GT value '%s'", genotype))
    }
    if (identical(allele, ".")) {
      missing <- TRUE
      next
    }
    if (!grepl("^[0-9]+$", allele)) {
      variant_malformed(line_number, sprintf("invalid GT allele '%s' in '%s'", allele, genotype))
    }
    index <- as.numeric(allele)
    if (index > alternate_count) {
      variant_malformed(line_number, sprintf(
        "GT allele index %d exceeds the %d alternate alleles",
        as.integer(index), as.integer(alternate_count)))
    }
    if (index != 0) alternate_alleles <- alternate_alleles + 1
  }
  list(missing = missing, alternate_alleles = alternate_alleles)
}

variant_genotype_counts <- function(columns, alternate_alleles, header, line_number) {
  if (header$sample_count == 0L) {
    return(list(missing = 0, called = 0, carriers = 0, alternate = 0))
  }
  format_fields <- strsplit(columns[[9L]], ":", fixed = TRUE)[[1L]]
  if (any(!nzchar(format_fields))) {
    variant_malformed(line_number, "FORMAT contains an empty field name")
  }
  if (anyDuplicated(format_fields) != 0L) {
    variant_malformed(line_number, sprintf(
      "FORMAT contains duplicate field name '%s'",
      format_fields[[which(duplicated(format_fields))[[1L]]]]))
  }
  gt_index <- match("GT", format_fields, nomatch = 0L)
  if (gt_index == 0L) {
    return(list(missing = 0, called = 0, carriers = 0, alternate = 0))
  }
  if (gt_index != 1L) {
    variant_malformed(line_number, "GT must be the first FORMAT field when present")
  }
  missing <- called <- carriers <- alternate <- 0
  samples <- if (length(columns) > 9L) columns[-(1:9)] else character(0)
  for (sample_index in seq_along(samples)) {
    sample <- samples[[sample_index]]
    if (!nzchar(sample)) {
      variant_malformed(line_number, sprintf("sample column %d is empty", sample_index))
    }
    values <- strsplit(sample, ":", fixed = TRUE)[[1L]]
    if (length(values) > length(format_fields)) {
      variant_malformed(line_number, sprintf(
        "sample column %d has more values than FORMAT declares", sample_index))
    }
    genotype <- if (length(values) >= gt_index) values[[gt_index]] else "."
    summary <- variant_genotype_summary(genotype, length(alternate_alleles), line_number)
    if (summary$missing) {
      missing <- missing + 1
    } else {
      called <- called + 1
      alternate <- alternate + summary$alternate_alleles
      if (summary$alternate_alleles != 0) carriers <- carriers + 1
    }
  }
  list(missing = missing, called = called, carriers = carriers, alternate = alternate)
}

variant_read_lines <- function(path) {
  probe <- file(path, "rb")
  on.exit(close(probe), add = TRUE)
  magic <- readBin(probe, "raw", n = 2L)
  connection <- if (length(magic) == 2L && identical(magic, as.raw(c(0x1f, 0x8b)))) {
    gzfile(path, "rt")
  } else {
    file(path, "rt")
  }
  on.exit(close(connection), add = TRUE)
  lines <- readLines(connection, warn = FALSE)
  sub("\r$", "", lines)
}

variant_ratio_if_nonzero <- function(numerator, denominator) {
  if (denominator == 0) NA_real_ else numerator / denominator
}

variant_stats_run <- function(path) {
  lines <- variant_read_lines(path)

  header <- NULL
  saw_file_format <- FALSE
  record_count <- sample_count <- pass_count <- filtered_count <- 0
  snp_count <- indel_count <- mnv_count <- symbolic_count <- 0
  multiallelic_count <- transition_count <- transversion_count <- 0
  missing_total <- called_total <- carriers_total <- alternate_total <- 0
  contigs <- list()

  for (line_number in seq_along(lines)) {
    line <- lines[[line_number]]
    if (is.null(header)) {
      if (line_number == 1L) {
        variant_validate_file_format(line)
        saw_file_format <- TRUE
        next
      }
      if (startsWith(line, "##")) {
        if (startsWith(line, "##fileformat=")) {
          variant_invalid_header(line_number,
            "the fileformat declaration must appear exactly once as the first line")
        }
        variant_validate_meta_line(line, line_number)
        next
      }
      if (startsWith(line, "#")) {
        if (!saw_file_format) {
          variant_invalid_header(line_number, "missing fileformat declaration")
        }
        header <- variant_parse_column_header(line, line_number)
        sample_count <- header$sample_count
        next
      }
      variant_invalid_header(line_number,
        "record data appears before the #CHROM column header")
    }

    if (startsWith(line, "#") || !nzchar(line)) {
      variant_malformed(line_number, if (!nzchar(line)) {
        "blank lines are not permitted after the column header"
      } else {
        "header or comment line appears after the #CHROM column header"
      })
    }

    columns <- strsplit(line, "\t", fixed = TRUE)[[1L]]
    if (length(columns) < 8L) {
      variant_malformed(line_number, sprintf(
        "expected at least 8 tab-separated columns, found %d", length(columns)))
    }
    if (length(columns) != header$column_count) {
      variant_malformed(line_number, sprintf(
        "expected %d columns to match the header, found %d",
        header$column_count, length(columns)))
    }
    if (!nzchar(columns[[1L]]) || identical(columns[[1L]], ".")) {
      variant_malformed(line_number, "CHROM must identify a contig")
    }
    if (!grepl("^[0-9]+$", columns[[2L]]) ||
        as.numeric(columns[[2L]]) <= 0 ||
        as.numeric(columns[[2L]]) > 1.8446744073709552e19) {
      variant_malformed(line_number, sprintf("invalid POS value '%s'", columns[[2L]]))
    }
    if (!nzchar(columns[[4L]]) || identical(columns[[4L]], ".")) {
      variant_malformed(line_number, "REF must contain an allele")
    }
    if (!nzchar(columns[[5L]])) {
      variant_malformed(line_number, "ALT must contain an allele or '.'")
    }
    if (!nzchar(columns[[7L]])) {
      variant_malformed(line_number, "FILTER must contain PASS, '.', or a filter name")
    }

    alleles <- variant_parse_alternates(columns[[5L]], line_number)
    genotype <- variant_genotype_counts(columns, alleles, header, line_number)

    record_count <- record_count + 1
    if (identical(columns[[7L]], "PASS")) {
      pass_count <- pass_count + 1
    } else if (!identical(columns[[7L]], ".")) {
      filtered_count <- filtered_count + 1
    }
    if (length(alleles) > 1L) multiallelic_count <- multiallelic_count + 1

    for (allele in alleles) {
      class <- variant_classify_allele(columns[[4L]], allele)
      if (class == "snp") snp_count <- snp_count + 1
      else if (class == "indel") indel_count <- indel_count + 1
      else if (class == "mnv") mnv_count <- mnv_count + 1
      else symbolic_count <- symbolic_count + 1
    }

    if (length(alleles) == 1L &&
        variant_classify_allele(columns[[4L]], alleles[[1L]]) == "snp") {
      kind <- variant_substitution_kind(columns[[4L]], alleles[[1L]])
      if (identical(kind, "transition")) transition_count <- transition_count + 1
      else if (identical(kind, "transversion")) transversion_count <- transversion_count + 1
    }

    missing_total <- missing_total + genotype$missing
    called_total <- called_total + genotype$called
    carriers_total <- carriers_total + genotype$carriers
    alternate_total <- alternate_total + genotype$alternate
    contig <- columns[[1L]]
    contigs[[contig]] <- if (is.null(contigs[[contig]])) 1 else contigs[[contig]] + 1
  }

  if (is.null(header)) {
    implementation_error("VCF column header is missing")
  }

  ti_tv_ratio <- variant_ratio_if_nonzero(transition_count, transversion_count)
  genotype_total <- missing_total + called_total
  missing_rate <- variant_ratio_if_nonzero(missing_total, genotype_total)

  warnings <- character(0)
  if (sample_count == 0) warnings <- c(warnings, NO_SAMPLES_WARNING)
  if (record_count == 0) warnings <- c(warnings, NO_RECORDS_WARNING)

  if (length(contigs) == 0L) {
    contig_counts <- list()
  } else {
    contigs <- contigs[order(names(contigs))]
    contig_counts <- as.list(contigs)
    for (name in names(contig_counts)) {
      contig_counts[[name]] <- as.numeric(contig_counts[[name]])
    }
  }

  list(
    record_count = record_count,
    sample_count = sample_count,
    pass_record_count = pass_count,
    filtered_record_count = filtered_count,
    snp_count = snp_count,
    indel_count = indel_count,
    mnv_count = mnv_count,
    symbolic_count = symbolic_count,
    multiallelic_record_count = multiallelic_count,
    transition_count = transition_count,
    transversion_count = transversion_count,
    ti_tv_ratio = ti_tv_ratio,
    missing_genotype_count = missing_total,
    called_genotype_count = called_total,
    carrier_genotype_count = carriers_total,
    alternate_allele_count = alternate_total,
    missing_genotype_rate = missing_rate,
    contig_counts = contig_counts,
    warnings = as.list(warnings)
  )
}

IMPLEMENTATION <- list(
  capability = CAPABILITY,
  input_roles = INPUT_ROLES,
  input_compression = INPUT_COMPRESSION,
  parameters = PARAMETERS,
  packages = character(0),
  software = function() list(),
  run = function(inputs, parameters) {
    variant_stats_run(inputs[["vcf"]])
  }
)
