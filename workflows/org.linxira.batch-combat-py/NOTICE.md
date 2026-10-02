# NOTICE

Algorithm reference (specification implemented by this pack, no upstream code
is copied or linked):

- Johnson, W. E., Li, C., & Rabinovic, A. (2007). Adjusting batch effects in
  microarray expression data using empirical Bayes methods. Biostatistics,
  8(1), 118-127. doi:10.1093/biostatistics/kxj037

This pack is an independent implementation of the published parametric
empirical-Bayes algorithm (posterior-mode fixed-point variant). The R backend
uses jsonlite (MIT) for JSON serialization only; the Python backend uses NumPy
(BSD-3-Clause).

## License

This pack is distributed under AGPL-3.0-or-later, matching the SDK license.

Runtime dependencies are installed separately from the pinned lock under
the SDK's project-isolated environment policy; dependencies are not vendored here.
