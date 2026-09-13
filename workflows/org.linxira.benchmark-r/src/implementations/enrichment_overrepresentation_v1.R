#!/usr/bin/env Rscript
# enrichment.overrepresentation.v1 - R pack implementation (M3 #6).
# Faithful port of the engine's ORA: association-union background, log-
# factorial hypergeometric upper tail with the engine's recurrence, and the
# engine's Benjamini-Hochberg tie-breaking. The engine is the specification;
# phyper is deliberately not used so parity is bitwise-comparable.

IMPLEMENTATION <- list(
  capability = "enrichment.overrepresentation.v1",
  input_roles = c("genes", "associations"),
  parameters = c("min_overlap", "max_terms", "include_genes"),
  packages = character(0),
  software = function() list(),
  run = function(inputs, parameters) {
    min_overlap <- as.integer(parameters$min_overlap %||% 1L)
    max_terms <- as.integer(parameters$max_terms %||% 100L)
    include_genes <- isTRUE(as.logical(parameters$include_genes %||% FALSE))
    if (is.na(min_overlap) || min_overlap < 1L) stop("min_overlap must be >= 1")
    if (is.na(max_terms) || max_terms < 1L) stop("max_terms must be >= 1")

    query <- read_gene_set(inputs$genes)
    associations <- read_associations(inputs$associations)
    background <- unique(unlist(associations$genes, use.names = FALSE))
    if (!length(background)) stop("no associations were found")
    mapped_query <- intersect(query, background)
    if (!length(mapped_query)) stop("none of the query identifiers occur in the association universe")

    population <- length(background)
    sample_size <- length(mapped_query)
    log_factorials <- c(0, cumsum(log(seq_len(population))))

    tested <- list()
    for (term_id in sort(associations$ids)) {
      overlap <- sort(intersect(associations$genes[[term_id]], mapped_query))
      if (length(overlap) < min_overlap) next
      term_count <- length(associations$genes[[term_id]])
      overlap_count <- length(overlap)
      p_value <- hypergeometric_upper_tail(population, term_count, sample_size,
                                           overlap_count, log_factorials)
      tested[[length(tested) + 1]] <- list(
        term_id = term_id,
        term_name = associations$names[[term_id]],
        namespace = associations$namespaces[[term_id]],
        overlap_count = overlap_count,
        query_gene_count = sample_size,
        background_term_count = term_count,
        background_gene_count = population,
        fold_enrichment = (overlap_count / sample_size) / (term_count / population),
        p_value = p_value,
        adjusted_p_value = 1,
        overlap_genes = if (include_genes) as.list(overlap) else list()
      )
    }
    if (!length(tested)) stop(sprintf("no terms meet min_overlap %d", min_overlap))
    p_values <- vapply(tested, function(t) t$p_value, numeric(1))
    term_ids <- vapply(tested, function(t) t$term_id, character(1))
    adjusted <- adjust_benjamini_hochberg(p_values, term_ids)
    for (index in seq_along(tested)) tested[[index]]$adjusted_p_value <- adjusted[index]
    rank_key <- order(vapply(tested, function(t) t$adjusted_p_value, numeric(1)),
                      p_values, -vapply(tested, function(t) t$overlap_count, numeric(1)),
                      term_ids, method = "radix")
    tested <- tested[rank_key]
    tested_term_count <- length(tested)
    reported <- tested[seq_len(min(max_terms, tested_term_count))]
    warnings <- character(0)
    unmapped <- length(query) - length(mapped_query)
    if (unmapped > 0) {
      warnings <- c(warnings, sprintf(
        "%d query identifiers were absent from the association universe", unmapped))
    }
    list(
      analysis_type = "custom",
      query_input_count = length(query),
      query_mapped_count = sample_size,
      query_unmapped_count = unmapped,
      background_gene_count = population,
      tested_term_count = tested_term_count,
      reported_term_count = length(reported),
      omitted_term_count = tested_term_count - length(reported),
      terms = reported,
      warnings = as.list(warnings)
    )
  }
)

MISSING <- c("", "-", "na", "n/a", "none", "null", ".")

is_missing_value <- function(value) tolower(trimws(value)) %in% MISSING

read_text_lines <- function(path) {
  connection <- if (grepl("\\.gz$", path)) gzfile(path, "rt") else file(path, "rt")
  lines <- readLines(connection, warn = FALSE)
  close(connection)
  lines
}

read_gene_set <- function(path) {
  lines <- trimws(read_text_lines(path))
  lines <- lines[nzchar(lines) & !startsWith(lines, "#")]
  if (!length(lines)) stop("query gene list is empty")
  delimiter <- if (grepl("\t", lines[1])) "\t" else if (grepl(",", lines[1])) "," else NA_character_
  gene_headers <- c("gene", "gene_id", "query", "query_id", "protein_id", "id")
  genes <- character(0)
  for (index in seq_along(lines)) {
    line <- lines[index]
    value <- trimws(if (is.na(delimiter)) line else strsplit(line, delimiter, fixed = TRUE)[[1]][1])
    if (index == 1 && tolower(value) %in% gene_headers) next
    if (!nzchar(value) || is_missing_value(value)) next
    genes <- c(genes, value)
  }
  if (!length(genes)) stop("query gene list is empty")
  unique(genes)
}

resolve_column <- function(headers, candidates, role) {
  lowered <- tolower(trimws(headers))
  for (candidate in candidates) {
    index <- match(candidate, lowered)
    if (!is.na(index)) return(index)
  }
  stop(sprintf("association table lacks the %s column", role))
}

optional_column <- function(headers, candidates) {
  lowered <- tolower(trimws(headers))
  for (candidate in candidates) {
    index <- match(candidate, lowered)
    if (!is.na(index)) return(index)
  }
  NA_integer_
}

optional_value <- function(record, column) {
  if (is.na(column) || column > length(record)) return(NULL)
  value <- trimws(record[column])
  if (!nzchar(value) || is_missing_value(value)) return(NULL)
  value
}

read_associations <- function(path) {
  lines <- read_text_lines(path)
  lines <- lines[nzchar(trimws(lines)) & !startsWith(trimws(lines), "#")]
  if (!length(lines)) stop("association table is empty")
  lowered_name <- tolower(path)
  delimiter <- if (grepl("\\.csv(\\.gz)?$", lowered_name)) "," else if (grepl("\\.(tsv|tab)(\\.gz)?$", lowered_name)) "\t" else if (grepl("\t", lines[1])) "\t" else if (grepl(",", lines[1])) "," else "\t"
  rows <- strsplit(lines, delimiter, fixed = TRUE)
  headers <- rows[[1]]
  gene_column <- resolve_column(headers, c("gene_id", "gene", "query", "locus"), "gene")
  term_column <- resolve_column(headers, c("term_id", "term", "go_id", "pathway_id", "category_id"), "term")
  name_column <- optional_column(headers, c("term_name", "name", "description"))
  namespace_column <- optional_column(headers, c("namespace", "category", "source"))
  term_ids <- character(0); term_names <- list(); term_namespaces <- list(); term_genes <- list()
  for (record in rows[-1]) {
    gene <- if (gene_column <= length(record)) trimws(record[gene_column]) else ""
    term_id <- if (term_column <= length(record)) trimws(record[term_column]) else ""
    if (!nzchar(gene) || is_missing_value(gene) || !nzchar(term_id) || is_missing_value(term_id)) next
    if (!term_id %in% term_ids) {
      term_ids <- c(term_ids, term_id)
      term_names[[term_id]] <- optional_value(record, name_column)
      term_namespaces[[term_id]] <- optional_value(record, namespace_column)
      term_genes[[term_id]] <- character(0)
    }
    term_genes[[term_id]] <- union(term_genes[[term_id]], gene)
  }
  if (!length(term_ids)) stop("no associations were found")
  list(ids = term_ids, names = term_names, namespaces = term_namespaces, genes = term_genes)
}

hypergeometric_upper_tail <- function(population, successes, draws, observed, log_factorials) {
  upper <- min(successes, draws)
  if (observed > upper) return(0)
  ln_choose <- function(total, selected) {
    if (selected > total) return(-Inf)
    log_factorials[total + 1] - log_factorials[selected + 1] - log_factorials[total - selected + 1]
  }
  log_probability <- ln_choose(successes, observed) +
    ln_choose(population - successes, draws - observed) -
    ln_choose(population, draws)
  probability <- exp(log_probability)
  total <- probability
  current <- observed
  while (current < upper) {
    numerator <- (successes - current) * (draws - current)
    remaining_failures <- population - successes
    sampled_failures <- draws - current
    denominator <- (current + 1) * (remaining_failures - sampled_failures + 1)
    if (denominator <= 0) break
    probability <- probability * numerator / denominator
    total <- total + probability
    current <- current + 1
  }
  min(max(total, 0), 1)
}

adjust_benjamini_hochberg <- function(p_values, term_ids) {
  count <- length(p_values)
  order <- order(p_values, term_ids, method = "radix")
  adjusted <- numeric(count)
  running <- 1
  for (reverse_index in seq(count, 1)) {
    term_index <- order[reverse_index]
    rank <- reverse_index
    candidate <- min(p_values[term_index] * count / rank, 1)
    running <- min(running, candidate)
    adjusted[term_index] <- running
  }
  adjusted
}

`%||%` <- function(left, right) if (is.null(left)) right else left
