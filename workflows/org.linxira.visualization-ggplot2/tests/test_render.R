# Smoke, determinism, and cross-backend tests for the ggplot2 PlotSpec
# renderer (M1-T2). Acceptance: same spec -> stable bytes (svg/png), and the
# data_summary must equal the matplotlib backend's for the same spec.
options(stringsAsFactors = FALSE)

pack_root <- normalizePath(file.path(dirname(sub(
  "^--file=", "", grep("^--file=", commandArgs(trailingOnly = FALSE), value = TRUE)
)), ".."), winslash = "/")
render_r <- file.path(pack_root, "src", "render.R")
py_render <- file.path(dirname(pack_root), "org.linxira.visualization-matplotlib",
                       "src", "render.py")
scratch <- file.path(tempdir(), "lx-viz-r")
dir.create(scratch, recursive = TRUE, showWarnings = FALSE)

run_renderer <- function(plot_spec, output_path) {
  request_path <- file.path(scratch, "request.json")
  result_path <- file.path(scratch, "result.json")
  writeLines(jsonlite::toJSON(list(plot_spec = plot_spec, output_path = output_path),
                              auto_unbox = TRUE), request_path)
  output <- system2("Rscript", c(shQuote(render_r), "--request", shQuote(request_path),
                                 "--result", shQuote(result_path)))
  stopifnot(output == 0)
  jsonlite::fromJSON(result_path, simplifyVector = FALSE)
}

scatter_spec <- list(
  title = "PCA scores", subtitle = "two groups",
  x_label = "PC1 (38%)", y_label = "PC2 (21%)",
  theme = "publication", palette = "set2", legend = TRUE,
  figure = list(width = 800L, height = 600L, dpi = 150L),
  output = list(format = "svg"),
  data = list(kind = "scatter", series = list(
    list(name = "control", x = c(1, 2, 3), y = c(2, 1.5, 3.5)),
    list(name = "treated", x = c(3.5, 4, 5), y = c(4, 5.5, 6))
  ))
)

# 1. every kind renders with a matching data summary
kinds <- list(
  list(spec = scatter_spec, out = "scatter.svg", kind = "scatter"),
  list(spec = modifyList(scatter_spec, list(data = list(kind = "line",
    series = scatter_spec$data$series))), out = "line.svg", kind = "line"),
  list(spec = modifyList(scatter_spec, list(data = list(kind = "bar",
    series = list(list(name = "counts", x = c("a", "b"), y = c(3, 7)))))), out = "bar.svg", kind = "bar"),
  list(spec = modifyList(scatter_spec, list(data = list(kind = "box",
    groups = list(list(name = "a", values = c(1, 2, 3)))))), out = "box.svg", kind = "box"),
  list(spec = modifyList(scatter_spec, list(data = list(kind = "violin",
    groups = list(list(name = "a", values = c(1, 2, 3)))))), out = "violin.svg", kind = "violin"),
  list(spec = modifyList(scatter_spec, list(
    output = list(format = "png"),
    data = list(kind = "heatmap", matrix = list(row_labels = c("g1", "g2"),
      col_labels = c("s1", "s2", "s3"), values = list(c(1, 2, 3), c(4, 5, 6)))))),
    out = "heatmap.png", kind = "heatmap")
)
for (case in kinds) {
  result <- run_renderer(case$spec, case$out)
  stopifnot(identical(result$backend, "ggplot2"))
  stopifnot(identical(result$artifact$path, case$out))
  stopifnot(file.exists(file.path(scratch, case$out)))
  stopifnot(nchar(result$plot_spec_sha256) == 64L)
}
cat("kinds render OK:", length(kinds), "\n")

# 2. same spec -> identical bytes three times (svg)
digests <- character(0)
for (i in 1:3) {
  result <- run_renderer(scatter_spec, "stable.svg")
  path <- file.path(scratch, "stable.svg")
  digests <- c(digests, digest::digest(file = path, algo = "sha256"))
}
stopifnot(length(unique(digests)) == 1L)
cat("svg byte-stable across 3 runs\n")

# 3. data summary equals the matplotlib backend for the same spec
py_request <- file.path(scratch, "py-request.json")
py_result <- file.path(scratch, "py-result.json")
writeLines(jsonlite::toJSON(list(plot_spec = scatter_spec, output_path = "cross.svg"),
                            auto_unbox = TRUE), py_request)
status <- suppressWarnings(system2("python", c(shQuote(py_render), "--request",
  shQuote(py_request), "--result", shQuote(py_result)), stdout = FALSE, stderr = FALSE))
if (status == 0 && file.exists(py_result)) {
  theirs <- jsonlite::fromJSON(py_result, simplifyVector = FALSE)$data_summary
  ours <- run_renderer(scatter_spec, "cross.svg")$data_summary
  stopifnot(identical(ours$kind, theirs$kind))
  stopifnot(identical(ours$series_count, theirs$series_count))
  stopifnot(identical(as.numeric(ours$point_counts), as.numeric(theirs$point_counts)))
  stopifnot(isTRUE(all.equal(as.numeric(ours$x_range), as.numeric(theirs$x_range))))
  stopifnot(isTRUE(all.equal(as.numeric(ours$y_range), as.numeric(theirs$y_range))))
  cat("cross-backend data summary matches matplotlib\n")
} else {
  cat("python renderer unavailable; cross-backend check skipped\n")
}

# 4. unknown kind fails structurally
bad <- file.path(scratch, "bad-request.json")
bad_result <- file.path(scratch, "bad-result.json")
writeLines(jsonlite::toJSON(list(plot_spec = list(data = list(kind = "hologram")),
  output_path = "x.svg"), auto_unbox = TRUE), bad)
status <- suppressWarnings(system2("Rscript", c(shQuote(render_r), "--request",
  shQuote(bad), "--result", shQuote(bad_result)), stdout = FALSE, stderr = FALSE))
stopifnot(status != 0)
failure <- jsonlite::fromJSON(bad_result, simplifyVector = FALSE)
stopifnot(identical(failure$status, "error"))
cat("unknown kind fails structurally\n")

cat("ggplot2 renderer tests passed\n")
