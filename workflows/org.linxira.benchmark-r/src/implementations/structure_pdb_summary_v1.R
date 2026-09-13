# structure.pdb.summary.v1 — independent base-R implementation.
#
# A field-for-field port of the engine's PDB summary parser (same fixed-width
# column extraction, MODEL/ENDMDL state machine, residue/chain grouping in
# first-appearance order with chains sorted by id, the same element inference
# table, the same AlphaFold pLDDT bands 90/70/50, and the same warnings).
# bio3d deliberately takes a different object-model view (altloc handling and
# hetflag residue ids differ from the engine), which would break the
# field-level comparison this backend exists for; the engine's parser IS the
# specification being cross-validated. substr() truncates past the end just
# like the engine's field() helper.

IMPLEMENTATION <- list(
  capability = "structure.pdb.summary.v1",
  input_roles = c("pdb"),
  parameters = c("interpret_b_factors_as_plddt"),
  packages = character(0),
  software = function() list(),
  run = function(inputs, parameters) {
    interpret_plddt <- isTRUE(parameters$interpret_b_factors_as_plddt)
    parse_pdb_summary(inputs[["pdb"]], interpret_plddt)
  }
)

field_at <- function(line, start, end) {
  # engine: line[start..end.min(len)] with 0-based half-open indexes;
  # substr is 1-based inclusive, so offsets shift by one.
  if (start + 1L > nchar(line, "bytes")) "" else substr(line, start + 1L, end)
}

optional_at <- function(line, start, end) {
  value <- trimws(field_at(line, start, end))
  if (nzchar(value)) value else NULL
}

required_at <- function(line, start, end, name, line_number) {
  value <- optional_at(line, start, end)
  if (is.null(value)) {
    implementation_error(paste0("malformed PDB record at line ", line_number, ": ", name, " is empty"))
  }
  value
}

number_at <- function(value, name, line_number) {
  parsed <- suppressWarnings(as.numeric(value))
  if (is.na(parsed)) {
    implementation_error(paste0(
      "malformed PDB record at line ", line_number, ": invalid ", name, " ",
      value, ": not a number"
    ))
  }
  if (!is.finite(parsed)) {
    implementation_error(paste0(
      "malformed PDB record at line ", line_number, ": invalid ", name, " ",
      value, ": must be finite"
    ))
  }
  parsed
}

optional_number_at <- function(line, start, end, name, line_number) {
  value <- optional_at(line, start, end)
  if (is.null(value)) NULL else number_at(value, name, line_number)
}

normalize_element <- function(value) {
  if (nchar(value) > 2L || !grepl("^[A-Za-z]+$", value)) {
    implementation_error(paste0("invalid element field ", value))
  }
  paste0(toupper(substr(value, 1L, 1L)), tolower(substr(value, 2L, nchar(value))))
}

infer_element <- function(raw_atom_name) {
  characters <- strsplit(raw_atom_name, "")[[1L]]
  position <- which(grepl("[A-Za-z]", characters))
  if (length(position) == 0L) return(NULL)
  first_index <- position[[1L]]
  first <- toupper(characters[[first_index]])
  if (first_index == 1L && length(characters) >= 2L && grepl("[A-Za-z]", characters[[2L]])) {
    candidate <- paste0(first, toupper(characters[[2L]]))
    if (candidate %in% c(
      "BR", "CA", "CD", "CL", "CO", "CU", "FE", "HG", "LI", "MG", "MN",
      "NA", "NI", "PB", "ZN"
    )) {
      return(paste0(substr(candidate, 1L, 1L), tolower(substr(candidate, 2L, 2L))))
    }
  }
  first
}

numeric_summary <- function(values, quantity) {
  if (length(values) == 0L) return(NULL)
  total <- 0
  for (value in values) {
    total <- total + value
    if (!is.finite(total)) {
      implementation_error(paste0(
        "PDB ", quantity, " exceeds the supported finite numeric range"
      ))
    }
  }
  mean_value <- total / length(values)
  if (!is.finite(mean_value)) {
    implementation_error(paste0(
      "PDB ", quantity, " exceeds the supported finite numeric range"
    ))
  }
  list(count = length(values), min = min(values), max = max(values), mean = mean_value)
}

parse_pdb_summary <- function(path, interpret_plddt) {
  magic <- readBin(path, "raw", n = 2L)
  source_bytes <- file.info(path)$size
  compressed <- length(magic) == 2L && identical(magic, as.raw(c(0x1f, 0x8b)))
  limit <- if (compressed) 64 * 1024 * 1024 else 128 * 1024 * 1024
  if (source_bytes > limit) {
    implementation_error(paste0("PDB source byte count exceeds the limit of ", limit))
  }
  connection <- if (compressed) gzfile(path, "rt") else file(path, "rt")
  lines <- tryCatch(readLines(connection, warn = FALSE), finally = close(connection))

  atoms <- list()
  residue_order <- list()
  residue_positions <- new.env(hash = TRUE, parent = emptyenv())
  chain_keys <- list()
  model_order <- character(0)
  seen_models <- character(0)
  current_model <- NULL
  explicit_models <- FALSE
  inferred_count <- 0L
  missing_count <- 0L
  warnings_out <- list()
  result_record_count <- 0L

  reserve <- function(additional) {
    result_record_count <<- result_record_count + additional
    if (result_record_count > 300000L) {
      implementation_error("PDB result record count exceeds the limit of 300000")
    }
  }

  line_number <- 0L
  for (raw_line in lines) {
    line_number <- line_number + 1L
    line <- sub("[\r\n]+$", "", raw_line)
    if (!identical(iconv(line, "UTF-8", "ASCII"), line)) {
      implementation_error(paste0(
        "malformed PDB record at line ", line_number,
        ": records must use ASCII fixed-width fields"
      ))
    }
    record <- trimws(field_at(line, 0L, 6L))
    if (identical(record, "END")) break
    if (identical(record, "MODEL")) {
      if (!is.null(current_model)) {
        implementation_error(paste0(
          "malformed PDB record at line ", line_number,
          ": nested MODEL records are not permitted"
        ))
      }
      if (!explicit_models && length(atoms) > 0L) {
        implementation_error(paste0(
          "malformed PDB record at line ", line_number,
          ": MODEL appears after coordinates that were not enclosed by a model"
        ))
      }
      explicit_models <- TRUE
      model_id <- optional_at(line, 10L, 14L)
      model_id <- if (is.null(model_id)) as.character(length(model_order) + 1L) else model_id
      if (model_id %in% seen_models) {
        implementation_error(paste0(
          "malformed PDB record at line ", line_number, ": duplicate MODEL identifier ",
          model_id
        ))
      }
      seen_models <- c(seen_models, model_id)
      reserve(1L)
      model_order <- c(model_order, model_id)
      current_model <- model_id
    } else if (identical(record, "ENDMDL")) {
      if (!explicit_models || is.null(current_model)) {
        implementation_error(paste0(
          "malformed PDB record at line ", line_number,
          ": ENDMDL does not close an active MODEL"
        ))
      }
      current_model <- NULL
    } else if (identical(record, "ATOM") || identical(record, "HETATM")) {
      if (explicit_models) {
        if (is.null(current_model)) {
          implementation_error(paste0(
            "malformed PDB record at line ", line_number,
            ": coordinate appears outside an explicit MODEL"
          ))
        }
        model_id <- current_model
      } else {
        if (length(model_order) == 0L) {
          reserve(1L)
          model_order <- "1"
          seen_models <- "1"
        }
        model_id <- "1"
      }
      if (length(atoms) >= 100000L) {
        implementation_error("PDB atom count exceeds the limit of 100000")
      }
      if (nchar(line, "bytes") < 54L) {
        implementation_error(paste0(
          "malformed PDB record at line ", line_number, ": ", record,
          " record is shorter than the coordinate columns"
        ))
      }
      atom <- list(
        index = length(atoms),
        serial = required_at(line, 6L, 11L, "atom serial", line_number),
        record = if (identical(record, "ATOM")) "atom" else "hetatm",
        model_id = model_id,
        residue_index = 0L,
        name = required_at(line, 12L, 16L, "atom name", line_number),
        alternate_location = optional_at(line, 16L, 17L) %||% NA_character_,
        residue_name = required_at(line, 17L, 20L, "residue name", line_number),
        chain_id = trimws(field_at(line, 21L, 22L)),
        residue_sequence_number = required_at(line, 22L, 26L, "residue sequence number", line_number),
        insertion_code = optional_at(line, 26L, 27L) %||% NA_character_,
        position = list(
          x = number_at(required_at(line, 30L, 38L, "x coordinate", line_number), "x coordinate", line_number),
          y = number_at(required_at(line, 38L, 46L, "y coordinate", line_number), "y coordinate", line_number),
          z = number_at(required_at(line, 46L, 54L, "z coordinate", line_number), "z coordinate", line_number)
        ),
        occupancy = optional_number_at(line, 54L, 60L, "occupancy", line_number) %||% NA_real_,
        b_factor = optional_number_at(line, 60L, 66L, "B-factor", line_number) %||% NA_real_,
        element = {
          raw_element <- optional_at(line, 76L, 78L)
          if (is.null(raw_element)) NA_character_ else normalize_element(raw_element)
        },
        formal_charge = optional_at(line, 78L, 80L) %||% NA_character_
      )
      residue_key <- list(
        atom$model_id, atom$chain_id, atom$residue_sequence_number,
        atom$insertion_code %||% NA_character_, atom$residue_name,
        identical(atom$record, "hetatm")
      )
      key_id <- paste(residue_key, collapse = "\r")
      is_new_residue <- !exists(key_id, envir = residue_positions, inherits = FALSE)
      chain_id_key <- paste(atom$model_id, atom$chain_id, sep = "\r")
      is_new_chain <- is.null(chain_keys[[chain_id_key]])
      reserve(1L + as.integer(is_new_residue) + as.integer(is_new_chain))
      if (is_new_chain) chain_keys[[chain_id_key]] <- TRUE
      if (is_new_residue) {
        residue_order[[length(residue_order) + 1L]] <- list(
          key = residue_key, atom_count = 0L, b_factor_sum = 0, b_factor_count = 0L
        )
        # 1-based position in residue_order; the envelope index is 0-based.
        assign(key_id, length(residue_order), envir = residue_positions)
      }
      residue_index <- get(key_id, envir = residue_positions, inherits = FALSE)
      residue <- residue_order[[residue_index]]
      residue$atom_count <- residue$atom_count + 1L
      if (!is.null(atom$b_factor)) {
        total <- residue$b_factor_sum + atom$b_factor
        if (!is.finite(total)) {
          implementation_error(paste0(
            "malformed PDB record at line ", line_number,
            ": residue B-factor sum exceeds the supported finite numeric range"
          ))
        }
        residue$b_factor_sum <- total
        residue$b_factor_count <- residue$b_factor_count + 1L
      }
      residue_order[[residue_index]] <- residue
      atom$residue_index <- residue_index - 1L
      if (is.na(atom$element)) {
        inferred <- infer_element(field_at(line, 12L, 16L))
        if (!is.null(inferred)) {
          atom$element <- inferred
          inferred_count <- inferred_count + 1L
        } else {
          missing_count <- missing_count + 1L
        }
      }
      atoms[[length(atoms) + 1L]] <- atom
    }
  }

  if (length(atoms) == 0L) implementation_error("PDB contains no ATOM or HETATM records")
  if (explicit_models && !is.null(current_model)) {
    warnings_out <- c(warnings_out, "the final MODEL has no ENDMDL record")
  }
  if (inferred_count > 0L) {
    warnings_out <- c(warnings_out, paste0(
      "inferred elements from atom-name alignment for ", inferred_count, " atoms"
    ))
  }
  if (missing_count > 0L) {
    warnings_out <- c(warnings_out, paste0(
      "could not determine an element for ", missing_count, " atoms"
    ))
  }

  minimum <- atoms[[1L]]$position
  maximum <- atoms[[1L]]$position
  for (atom in atoms[-1L]) {
    minimum <- list(x = min(minimum$x, atom$position$x), y = min(minimum$y, atom$position$y), z = min(minimum$z, atom$position$z))
    maximum <- list(x = max(maximum$x, atom$position$x), y = max(maximum$y, atom$position$y), z = max(maximum$z, atom$position$z))
  }
  span <- list()
  center <- list()
  for (axis in c("x", "y", "z")) {
    difference <- maximum[[axis]] - minimum[[axis]]
    if (!is.finite(difference)) {
      implementation_error(paste0("PDB ", axis, "-coordinate span exceeds the supported finite numeric range"))
    }
    span[[axis]] <- difference
    center_value <- minimum[[axis]] + difference / 2.0
    if (!is.finite(center_value)) {
      implementation_error(paste0("PDB ", axis, "-coordinate center exceeds the supported finite numeric range"))
    }
    center[[axis]] <- center_value
  }

  b_factors <- vapply(atoms, function(atom) atom$b_factor, numeric(1))
  b_factor_summary <- numeric_summary(b_factors[!is.na(b_factors)], "B-factor summary")

  output_residues <- vector("list", length(residue_order))
  for (index in seq_along(residue_order)) {
    key <- residue_order[[index]]$key
    output_residues[[index]] <- list(
      index = index - 1L,
      model_id = key[[1L]],
      chain_id = key[[2L]],
      sequence_number = key[[3L]],
      insertion_code = key[[4L]],
      name = key[[5L]],
      is_hetero = key[[6L]],
      atom_count = residue_order[[index]]$atom_count,
      plddt = NA_real_
    )
  }

  alphafold_confidence <- NULL
  if (interpret_plddt) {
    for (atom in atoms) {
      if (!identical(atom$record, "atom")) next
      value <- atom$b_factor
      if (is.na(value)) {
        implementation_error(paste0(
          "malformed PDB record: polymer atom ", atom$serial,
          " lacks the B-factor required for pLDDT interpretation"
        ))
      }
      if (value < 0 || value > 100) {
        implementation_error(paste0(
          "malformed PDB record: polymer atom ", atom$serial, " has B-factor ",
          format(value, digits = 17), ", outside the pLDDT range 0..100"
        ))
      }
    }
    values <- list()
    bands <- list(very_high_count = 0L, confident_count = 0L, low_count = 0L, very_low_count = 0L)
    for (index in seq_along(residue_order)) {
      residue <- residue_order[[index]]
      if (isTRUE(residue$key[[6L]])) next
      if (residue$b_factor_count != residue$atom_count) {
        implementation_error(paste0(
          "malformed PDB record: residue ", residue$key[[5L]], " ", residue$key[[3L]],
          " lacks complete B-factor values"
        ))
      }
      value <- residue$b_factor_sum / residue$b_factor_count
      output_residues[[index]]$plddt <- value
      values <- c(values, value)
      if (value >= 90) {
        bands$very_high_count <- bands$very_high_count + 1L
      } else if (value >= 70) {
        bands$confident_count <- bands$confident_count + 1L
      } else if (value >= 50) {
        bands$low_count <- bands$low_count + 1L
      } else {
        bands$very_low_count <- bands$very_low_count + 1L
      }
    }
    summary <- numeric_summary(unlist(values), "AlphaFold pLDDT summary")
    if (is.null(summary)) {
      implementation_error(
        "malformed PDB record: AlphaFold pLDDT interpretation requires at least one ATOM residue"
      )
    }
    alphafold_confidence <- list(
      source = "pdb-b-factor-explicit",
      residue_count = summary$count,
      min_plddt = summary$min,
      max_plddt = summary$max,
      mean_plddt = summary$mean,
      bands = bands
    )
    warnings_out <- c(warnings_out, paste(
      "B-factor values were interpreted as AlphaFold pLDDT because the caller explicitly",
      "requested it; PDB content alone does not establish AlphaFold provenance"
    ))
  }

  models <- vector("list", length(model_order))
  for (position in seq_along(model_order)) {
    model_id <- model_order[[position]]
    model_atoms <- Filter(function(atom) identical(atom$model_id, model_id), atoms)
    model_residues <- Filter(function(residue) identical(residue$model_id, model_id), output_residues)
    chain_ids <- sort(vapply(model_residues, function(residue) residue$chain_id, character(1)), method = "radix")
    chain_ids <- unique(chain_ids)
    chains <- vector("list", length(chain_ids))
    for (chain_position in seq_along(chain_ids)) {
      chain_id <- chain_ids[[chain_position]]
      chain_residues <- Filter(function(residue) identical(residue$chain_id, chain_id), model_residues)
      chains[[chain_position]] <- list(
        chain_id = chain_id,
        atom_count = sum(vapply(model_atoms, function(atom) identical(atom$chain_id, chain_id), logical(1))),
        residue_count = length(chain_residues),
        polymer_residue_count = sum(vapply(chain_residues, function(residue) !isTRUE(residue$is_hetero), logical(1))),
        hetero_residue_count = sum(vapply(chain_residues, function(residue) isTRUE(residue$is_hetero), logical(1)))
      )
    }
    models[[position]] <- list(
      model_id = model_id,
      atom_count = length(model_atoms),
      residue_count = length(model_residues),
      chains = chains
    )
  }

  element_counts <- table(vapply(atoms, function(atom) if (is.na(atom$element)) "unknown" else atom$element, character(1)))
  sorted_elements <- sort(names(element_counts), method = "radix")
  element_counts_out <- setNames(as.list(unname(element_counts[sorted_elements])), sorted_elements)
  polymer_atom_count <- sum(vapply(atoms, function(atom) identical(atom$record, "atom"), logical(1)))

  list(
    format = "pdb",
    coordinate_units = "angstrom",
    model_count = length(models),
    chain_count = sum(vapply(models, function(model) length(model$chains), integer(1))),
    residue_count = length(output_residues),
    atom_count = length(atoms),
    polymer_atom_count = polymer_atom_count,
    hetero_atom_count = length(atoms) - polymer_atom_count,
    element_counts = element_counts_out,
    bounds = list(min = minimum, max = maximum, center = center, span = span),
    b_factor_summary = b_factor_summary,
    alphafold_confidence = alphafold_confidence,
    models = models,
    residues = output_residues,
    atoms = atoms,
    warnings = as.list(unlist(warnings_out))
  )
}

`%||%` <- function(left, right) if (is.null(left)) right else left
