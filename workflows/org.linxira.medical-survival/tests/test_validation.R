# Validate the survival pack request layer without fitting models.
# Usage: Rscript workflows/org.linxira.medical-survival/tests/test_validation.R
source(file.path("workflows/org.linxira.medical-survival/src/run_survival.R"))

expect_error <- function(expr, pattern) {
  message <- tryCatch(
    {
      force(expr)
      NULL
    },
    error = function(error) conditionMessage(error)
  )
  if (is.null(message) || !grepl(pattern, message, perl = TRUE)) {
    stop(sprintf(
      "expected an error matching %s but got: %s",
      pattern, if (is.null(message)) "<no error>" else message
    ))
  }
}

base_document <- list(
  schema_version = "2",
  job_id = "validation",
  capability = "medical.survival.v1",
  inputs = list(list(
    artifact_id = "cohort",
    role = "cohort",
    cardinality = "single",
    files = list(list(file_id = "cohort-1", path = "cohort.csv", format = "csv", compression = "none", size_bytes = 1))
  )),
  execution = list(mode = "local-cpu"),
  parameters = list(
    output_directory = "out",
    time_column = "time",
    event_column = "event",
    group_column = "group",
    reference_level = "control"
  )
)

document <- base_document
config <- validate_request(document, "out/result.json")
stopifnot(config$time_column == "time", config$reference_level == "control")

expect_error(
  validate_request(utils::modifyList(base_document, list(schema_version = "1")), "x"),
  "schema_version"
)
expect_error(
  validate_request(utils::modifyList(base_document, list(capability = "other.v1")), "x"),
  "capability"
)
bad_document <- base_document
bad_document$parameters <- list(
  output_directory = "out", time_column = "time", event_column = "event",
  group_column = "group"
)
expect_error(
  validate_request(bad_document, "x"),
  "reference_level"
)
no_input_document <- base_document
no_input_document$inputs <- list()
expect_error(
  validate_request(no_input_document, "x"),
  "exactly one input"
)

cat("survival workflow validation tests passed\n")
