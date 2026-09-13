#!/usr/bin/env Rscript
# Deterministic ggplot2 renderer for Linxira PlotSpec (M1-T2).
#
# Same request/result contract as the matplotlib pack: request JSON
# {"plot_spec": ..., "output_path": "<relative>"} renders the figure and
# writes a render-result JSON whose data_summary (point counts, coordinate
# ranges) must match the matplotlib backend for the same spec. svglite and
# cairo png produce timestamp-free bytes, so identical specs are
# byte-identical; base pdf embeds a creation date, which is disclosed in
# warnings instead of hidden.

suppressPackageStartupMessages({
  library(jsonlite)
  library(ggplot2)
  library(svglite)
  library(digest)
})

schema_version <- "1"
backend_name <- "ggplot2"

canonical_hash <- function(value) {
  digest_json <- jsonlite::toJSON(value, auto_unbox = TRUE, digits = NA, null = "null")
  digest::digest(as.character(digest_json), algo = "sha256", serialize = FALSE)
}

finite <- function(values) {
  values[vapply(values, function(v) is.finite(suppressWarnings(as.numeric(v))), logical(1))]
}

numeric_or_null <- function(x) if (length(x) == 0) NULL else as.numeric(x)

data_summary <- function(data) {
  kind <- data$kind %||% ""
  if (kind %in% c("scatter", "line", "bar")) {
    series <- data$series %||% list()
    xs <- numeric(0); ys <- numeric(0); counts <- integer(0)
    for (entry in series) {
      xs <- c(xs, finite(entry$x %||% list()))
      ys <- c(ys, finite(entry$y %||% list()))
      counts <- c(counts, min(length(finite(entry$x %||% list())), length(finite(entry$y %||% list()))))
    }
    return(list(kind = kind, series_count = length(series),
                point_counts = as.integer(counts),
                x_range = if (length(xs)) range(xs) else NULL,
                y_range = if (length(ys)) range(ys) else NULL))
  }
  if (kind %in% c("box", "violin")) {
    groups <- data$groups %||% list()
    values <- numeric(0); counts <- integer(0)
    for (group in groups) {
      group_values <- finite(group$values %||% list())
      values <- c(values, group_values)
      counts <- c(counts, length(group_values))
    }
    return(list(kind = kind, series_count = length(groups),
                point_counts = as.integer(counts),
                x_range = NULL,
                y_range = if (length(values)) range(values) else NULL))
  }
  if (identical(kind, "heatmap")) {
    matrix_values <- data$matrix$values
    flat <- finite(unlist(matrix_values, use.names = FALSE))
    return(list(kind = kind, series_count = 1L,
                point_counts = nrow(matrix_values) * ncol(matrix_values),
                x_range = NULL,
                y_range = if (length(flat)) range(flat) else NULL))
  }
  stop(sprintf("unsupported plot kind '%s'", kind))
}

build_frame <- function(data) {
  kind <- data$kind %||% ""
  if (kind %in% c("scatter", "line", "bar")) {
    rows <- list()
    for (index in seq_along(data$series %||% list())) {
      entry <- data$series[[index]]
      rows[[length(rows) + 1]] <- data.frame(
        x = as.character(unlist(entry$x %||% list(), use.names = FALSE)),
        x_num = suppressWarnings(as.numeric(unlist(entry$x %||% list(), use.names = FALSE))),
        y = as.numeric(unlist(entry$y %||% list(), use.names = FALSE)),
        series = entry$name %||% sprintf("series %d", index)
      )
    }
    return(do.call(rbind, rows))
  }
  if (kind %in% c("box", "violin")) {
    rows <- list()
    for (index in seq_along(data$groups %||% list())) {
      entry <- data$groups[[index]]
      rows[[length(rows) + 1]] <- data.frame(
        group = entry$name %||% sprintf("group %d", index),
        value = as.numeric(finite(entry$values %||% list()))
      )
    }
    return(do.call(rbind, rows))
  }
  NULL
}

apply_theme <- function(plot, spec) {
  theme_name <- spec$theme %||% "light"
  base <- ggplot2::theme_minimal(base_size = spec$font$size %||% 12)
  if (identical(theme_name, "dark")) {
    base <- base + ggplot2::theme(
      plot.background = ggplot2::element_rect(fill = "#1c1c1c", colour = "#1c1c1c"),
      panel.background = ggplot2::element_rect(fill = "#242424", colour = NA),
      text = ggplot2::element_text(colour = "white"),
      axis.text = ggplot2::element_text(colour = "white")
    )
  } else if (identical(theme_name, "publication")) {
    base <- base + ggplot2::theme(
      panel.grid.minor = ggplot2::element_blank(),
      text = ggplot2::element_text(family = spec$font$family %||% "serif")
    )
  }
  if (!isTRUE(spec$grid %||% TRUE)) {
    base <- base + ggplot2::theme(panel.grid = ggplot2::element_blank())
  }
  plot + base
}

render <- function(spec, output_path) {
  warnings <- character(0)
  data <- spec$data
  summary <- data_summary(data)
  kind <- data$kind %||% ""

  figure <- spec$figure %||% list()
  width <- figure$width %||% 800
  height <- figure$height %||% 600
  dpi <- figure$dpi %||% 150
  palette_name <- spec$palette %||% "set2"
  palette_map <- c(set2 = "Set2", set1 = "Set1", pastel = "Pastel1", vivid = "Set3")
  brewer_name <- palette_map[[tolower(palette_name)]]
  if (is.na(brewer_name)) {
    warnings <- c(warnings, sprintf("unknown palette '%s'; falling back to Set2", palette_name))
    brewer_name <- "Set2"
  }

  frame <- build_frame(data)
  if (kind %in% c("scatter", "line", "bar")) {
    frame$x_num[!is.finite(frame$x_num)] <- NA_real_
    if (identical(kind, "bar")) {
      plot <- ggplot2::ggplot(frame, ggplot2::aes(x = x, y = y, fill = series)) +
        ggplot2::geom_col(position = ggplot2::position_dodge()) +
        ggplot2::scale_fill_brewer(palette = brewer_name)
    } else {
      plot <- ggplot2::ggplot(frame, ggplot2::aes(x = x_num, y = y, colour = series)) +
        ggplot2::scale_colour_brewer(palette = brewer_name)
      if (identical(kind, "line")) {
        plot <- plot + ggplot2::geom_line(linewidth = 0.8)
      } else {
        plot <- plot + ggplot2::geom_point()
      }
    }
    if (isTRUE(spec$legend %||% TRUE)) plot <- plot else plot <- plot + ggplot2::theme(legend.position = "none")
  } else if (kind %in% c("box", "violin")) {
    frame$group <- factor(frame$group, levels = unique(frame$group))
    plot <- ggplot2::ggplot(frame, ggplot2::aes(x = group, y = value, fill = group)) +
      ggplot2::scale_fill_brewer(palette = brewer_name) +
      (if (identical(kind, "violin")) ggplot2::geom_violin() else ggplot2::geom_boxplot()) +
      ggplot2::theme(legend.position = "none")
  } else if (identical(kind, "heatmap")) {
    matrix_values <- matrix(as.numeric(unlist(data$matrix$values, use.names = FALSE)),
                            nrow = length(data$matrix$values), byrow = TRUE)
    long_frame <- expand.grid(row_index = seq_len(nrow(matrix_values)),
                              col_index = seq_len(ncol(matrix_values)))
    long_frame$value <- as.vector(matrix_values)
    plot <- ggplot2::ggplot(long_frame, ggplot2::aes(x = col_index, y = row_index, fill = value)) +
      ggplot2::geom_tile() +
      ggplot2::scale_fill_gradient2(low = "#3b4cc0", mid = "#f7f7f7", high = "#b40426",
                                    midpoint = mean(long_frame$value)) +
      ggplot2::scale_y_reverse() +
      ggplot2::coord_equal()
  }

  title_parts <- c(spec$title %||% "", spec$subtitle %||% "")
  title_parts <- title_parts[nzchar(title_parts)]
  if (length(title_parts)) plot <- plot + ggplot2::ggtitle(title_parts[1])
  x_label <- spec$x_label %||% data$x_label_hint %||% "x"
  y_label <- spec$y_label %||% data$y_label_hint %||% "y"
  plot <- plot + ggplot2::xlab(x_label) + ggplot2::ylab(y_label)
  if (isTRUE(spec$x_log %||% FALSE)) plot <- plot + ggplot2::scale_x_log10()
  if (isTRUE(spec$y_log %||% FALSE)) plot <- plot + ggplot2::scale_y_log10()
  if (!is.null(numeric_or_null(spec$x_range))) plot <- plot + ggplot2::xlim(spec$x_range)
  if (!is.null(numeric_or_null(spec$y_range))) plot <- plot + ggplot2::ylim(spec$y_range)
  plot <- apply_theme(plot, spec)

  fmt <- spec$output$format %||% "svg"
  if (!fmt %in% c("svg", "png", "pdf")) {
    warnings <- c(warnings, sprintf("unsupported format '%s'; rendered svg", fmt))
    fmt <- "svg"
  }
  if (identical(fmt, "pdf")) {
    warnings <- c(warnings, "pdf output embeds a creation date; byte-stable reruns are guaranteed for svg and png only")
  }
  output_path <- sub("\\.[^.]*$", paste0(".", fmt), output_path)
  ggplot2::ggsave(output_path, plot = plot,
                  width = width / dpi, height = height / dpi, dpi = dpi)

  list(
    schema_version = schema_version,
    backend = backend_name,
    plot_spec_sha256 = canonical_hash(spec),
    artifact = list(
      path = basename(output_path),
      format = fmt,
      width = width, height = height, dpi = dpi,
      bytes = as.integer(file.info(output_path)$size)
    ),
    data_summary = summary,
    warnings = warnings
  )
}

main <- function(argv) {
  request_path <- NULL; result_path <- NULL
  i <- 1L
  while (i <= length(argv)) {
    if (identical(argv[[i]], "--request")) request_path <- argv[[i + 1L]]
    if (identical(argv[[i]], "--result")) result_path <- argv[[i + 1L]]
    i <- i + 2L
  }
  if (is.null(request_path) || is.null(result_path)) {
    message("usage: render.R --request REQUEST --result RESULT")
    quit(status = 2L)
  }
  request <- jsonlite::fromJSON(request_path, simplifyVector = FALSE)
  spec <- request$plot_spec
  output_path <- file.path(dirname(request_path), request$output_path %||% "figure.svg")
  result <- tryCatch(render(spec, output_path), error = function(e) {
    list(schema_version = schema_version, backend = backend_name,
         status = "error", message = paste0(class(e)[1], ": ", conditionMessage(e)))
  })
  jsonlite::write_json(result, result_path, auto_unbox = TRUE, null = "null",
                       na = "null", digits = NA, pretty = TRUE)
  if (!is.null(result$status) && identical(result$status, "error")) quit(status = 1L)
  invisible(0L)
}

`%||%` <- function(left, right) if (is.null(left)) right else left

main(commandArgs(trailingOnly = TRUE))
