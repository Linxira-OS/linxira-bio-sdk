# set.venn.v1 — independent base-R implementation (exact set arithmetic).
#
# Counterpart of the Rust engine's Venn analysis
# (engine/crates/linxira-bio-core/src/set_analysis.rs): the membership table
# is parsed with the engine's rules (header names the sets, one item per
# non-empty cell, short rows accepted, a mask bit per column) and the
# summary is exact set arithmetic — per-set sizes, one intersection entry
# per occupancy mask ordered by mask bits, degree as popcount, and items
# sorted in C locale (the engine iterates its ordered map). VennDiagram is a
# plotting concern; the analysis itself needs no packages. R has no shift
# operator and Venn allows at most 6 columns, so the mask (<= 63) is held as
# an exactly-representable double and bits are tested arithmetically.

IMPLEMENTATION <- list(
  capability = "set.venn.v1",
  input_roles = c("table"),
  parameters = c("include_items", "max_intersections"),
  packages = character(0),
  software = function() list(),
  run = function(inputs, parameters) {
    include_items <- isTRUE(parameters$include_items)
    max_intersections <- as.integer(parameters$max_intersections %||% 50)
    if (is.na(max_intersections) || max_intersections < 1L || max_intersections > 10000L) {
      implementation_error("max_intersections must be between 1 and 10000")
    }
    table <- read_membership_table(inputs[["table"]])
    if (length(table$names) > 6L) {
      implementation_error(paste0(
        "Venn analysis accepts 2 to 6 set columns; found ", length(table$names)
      ))
    }
    summarize_memberships(table, include_items)
  }
)

`%||%` <- function(left, right) if (is.null(left)) right else left

count_char <- function(text, character) {
  matches <- gregexpr(character, text, fixed = TRUE)[[1L]]
  if (length(matches) == 1L && matches[[1L]] == -1L) 0L else length(matches)
}

has_bit <- function(mask, column) {
  power <- 2^(column - 1L)
  mask %% (2 * power) >= power
}

read_membership_table <- function(path) {
  name <- tolower(basename(path))
  if (endsWith(name, ".gz")) name <- substr(name, 1L, nchar(name) - 3L)
  if (endsWith(name, ".csv")) {
    delimiter <- ","
  } else if (endsWith(name, ".tsv") || endsWith(name, ".tab")) {
    delimiter <- "\t"
  } else {
    magic <- readBin(path, "raw", n = 2L)
    connection <- if (length(magic) == 2L && identical(magic, as.raw(c(0x1f, 0x8b)))) {
      gzfile(path, "rt")
    } else {
      file(path, "rt")
    }
    header <- tryCatch(readLines(connection, n = 1L, warn = FALSE), finally = close(connection))
    delimiter <- if (count_char(header, "\t") >= count_char(header, ",")) "\t" else ","
  }

  magic <- readBin(path, "raw", n = 2L)
  connection <- if (length(magic) == 2L && identical(magic, as.raw(c(0x1f, 0x8b)))) {
    gzfile(path, "rt")
  } else {
    file(path, "rt")
  }
  lines <- tryCatch(readLines(connection, warn = FALSE), finally = close(connection))
  nonblank <- lines[nzchar(trimws(lines))]
  if (length(nonblank) == 0L) implementation_error("set table is empty")
  split_lines <- lapply(nonblank, function(line) strsplit(line, delimiter, fixed = TRUE)[[1L]])

  names <- trimws(split_lines[[1L]])
  if (any(!nzchar(names))) implementation_error("set names must not be empty")
  if (anyDuplicated(names) != 0L) {
    implementation_error(paste0("duplicate set name ", names[[anyDuplicated(names)]]))
  }
  if (length(names) < 2L || length(names) > 64L) {
    implementation_error(paste0(
      "expected 2 to 64 named columns; found ", length(names)
    ))
  }

  item_masks <- list()
  for (row_number in seq_along(split_lines)[-1L]) {
    record <- split_lines[[row_number]]
    if (row_number - 1L > 1000000L) {
      implementation_error("row count exceeds the local analysis limit")
    }
    if (length(record) > length(names)) {
      implementation_error(paste0(
        "data row ", row_number, " has ", length(record),
        " fields but the header has ", length(names)
      ))
    }
    for (column in seq_along(record)) {
      item <- trimws(record[[column]])
      if (!nzchar(item)) next
      # One (item, column) pair appears at most once, so adding the column's
      # power is exactly the engine's mask |= 1 << column; the unique-item
      # limit applies only when a NEW item would be stored.
      if (is.null(item_masks[[item]]) && length(item_masks) >= 1000000L) {
        implementation_error("unique item count exceeds the local analysis limit")
      }
      item_masks[[item]] <- (item_masks[[item]] %||% 0) + 2^(column - 1L)
    }
  }
  if (length(item_masks) == 0L) {
    implementation_error("no non-empty set items were found")
  }
  list(names = names, masks = item_masks)
}

summarize_memberships <- function(table, include_items) {
  names <- table$names
  sizes <- integer(length(names))
  masks <- sort(unlist(table$masks, use.names = FALSE))
  intersections <- vector("list", length(masks))
  for (position in seq_along(masks)) {
    mask <- masks[[position]]
    member_items <- sort(
      names(table$masks)[unlist(table$masks, use.names = FALSE) == mask],
      method = "radix"
    )
    degree <- 0L
    for (column in seq_along(names)) {
      if (has_bit(mask, column)) {
        sizes[[column]] <- sizes[[column]] + 1L
        degree <- degree + 1L
      }
    }
    intersections[[position]] <- list(
      sets = as.list(names[vapply(seq_along(names), function(column) has_bit(mask, column), logical(1))]),
      degree = degree,
      count = length(member_items),
      items = if (isTRUE(include_items)) as.list(member_items) else list()
    )
  }
  list(
    set_count = length(names),
    union_size = length(table$masks),
    set_sizes = unname(Map(
      function(name, count) list(name = name, count = count),
      names,
      as.list(sizes)
    )),
    intersections = intersections
  )
}
