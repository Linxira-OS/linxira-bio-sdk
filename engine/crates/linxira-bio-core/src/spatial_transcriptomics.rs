use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

/// Summary statistics for a 10x Genomics sparse expression matrix.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SpatialTranscriptomicsResult {
    pub format: String,
    pub n_barcodes: u64,
    pub n_features: u64,
    pub n_nonzero: u64,
    pub total_counts: u64,
    pub counts_per_barcode: Vec<u64>,
    pub genes_per_barcode: Vec<u64>,
    pub mean_counts: Option<f64>,
    pub median_genes: Option<u64>,
    pub p90_genes: Option<u64>,
    pub barcode_rank: Vec<BarcodeRank>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BarcodeRank {
    pub rank: u64,
    pub barcode: String,
    pub total_counts: u64,
    pub n_genes: u64,
}

#[derive(Debug)]
pub enum SpatialError {
    Io(io::Error),
    MissingFeatureAnnotation,
    MissingBarcodeAnnotation,
    MalformedMatrix { line: usize, message: String },
}

impl Display for SpatialError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read 10x matrix: {error}"),
            Self::MissingFeatureAnnotation => {
                formatter.write_str("features/genes annotation file is required")
            }
            Self::MissingBarcodeAnnotation => {
                formatter.write_str("barcodes annotation file is required")
            }
            Self::MalformedMatrix { line, message } => {
                write!(formatter, "malformed matrix market line {line}: {message}")
            }
        }
    }
}

impl Error for SpatialError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SpatialError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn open_maybe_gzip(path: &Path) -> io::Result<Box<dyn Read>> {
    let mut magic = [0_u8; 2];
    let mut probe = File::open(path)?;
    let magic_length = probe.read(&mut magic)?;
    if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Ok(Box::new(MultiGzDecoder::new(File::open(path)?)))
    } else {
        Ok(Box::new(File::open(path)?))
    }
}

fn read_lines(path: &Path) -> io::Result<Vec<String>> {
    let reader = BufReader::new(open_maybe_gzip(path)?);
    reader.lines().collect()
}

/// Analyze a 10x Genomics sparse expression matrix (matrix.mtx[.gz] plus the
/// feature and barcode annotations).
pub fn spatial_transcriptomics_path(
    matrix_path: impl AsRef<Path>,
    features_path: impl AsRef<Path>,
    barcodes_path: impl AsRef<Path>,
) -> Result<SpatialTranscriptomicsResult, SpatialError> {
    let matrix_path = matrix_path.as_ref();
    let features_path = features_path.as_ref();
    let barcodes_path = barcodes_path.as_ref();
    if !features_path.is_file() {
        return Err(SpatialError::MissingFeatureAnnotation);
    }
    if !barcodes_path.is_file() {
        return Err(SpatialError::MissingBarcodeAnnotation);
    }
    let features = read_lines(features_path)?;
    let barcodes = read_lines(barcodes_path)?;
    if features.is_empty() || barcodes.is_empty() {
        return Err(SpatialError::MalformedMatrix {
            line: 1,
            message: "feature or barcode annotation is empty".to_owned(),
        });
    }

    let mut reader = BufReader::new(open_maybe_gzip(matrix_path)?);
    let mut result = SpatialTranscriptomicsResult {
        format: "10x matrix-market (MTX)".to_owned(),
        ..SpatialTranscriptomicsResult::default()
    };
    let mut line_number = 0_usize;
    let mut buffer = String::new();
    let mut header_seen = false;
    let mut dimensions: Option<(u64, u64)> = None;
    let mut count_entries = 0_u64;

    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader.read_line(&mut buffer).map_err(SpatialError::Io)?;
        if bytes_read == 0 {
            break;
        }
        let line = buffer.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        if !header_seen {
            if line.starts_with('%') {
                continue;
            }
            if line.starts_with("%%MatrixMarket") || line.starts_with("%%matrixmarket") {
                if !line.to_ascii_lowercase().contains("coordinate") {
                    return Err(SpatialError::MalformedMatrix {
                        line: line_number,
                        message: "unsupported matrix market format (coordinate expected)"
                            .to_owned(),
                    });
                }
                continue;
            }
            let mut parts = line.split_whitespace();
            let rows = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| SpatialError::MalformedMatrix {
                    line: line_number,
                    message: "invalid row count".to_owned(),
                })?;
            let columns = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| SpatialError::MalformedMatrix {
                    line: line_number,
                    message: "invalid column count".to_owned(),
                })?;
            let declared = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| SpatialError::MalformedMatrix {
                    line: line_number,
                    message: "invalid entry count".to_owned(),
                })?;
            if rows == 0 || columns == 0 {
                return Err(SpatialError::MalformedMatrix {
                    line: line_number,
                    message: "matrix dimensions must be positive".to_owned(),
                });
            }
            if rows as usize != features.len() || columns as usize != barcodes.len() {
                return Err(SpatialError::MalformedMatrix {
                    line: line_number,
                    message: format!(
                        "matrix dimensions {rows}x{columns} do not match annotations {}x{}",
                        features.len(),
                        barcodes.len()
                    ),
                });
            }
            dimensions = Some((rows, columns));
            result.n_barcodes = columns;
            result.n_features = rows;
            result.counts_per_barcode = vec![0_u64; columns as usize];
            result.genes_per_barcode = vec![0_u64; columns as usize];
            header_seen = true;
            count_entries = declared;
            continue;
        }

        let mut parts = line.split_whitespace();
        let row = parts
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| SpatialError::MalformedMatrix {
                line: line_number,
                message: "invalid row index".to_owned(),
            })?;
        let column = parts
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| SpatialError::MalformedMatrix {
                line: line_number,
                message: "invalid column index".to_owned(),
            })?;
        let value = parts
            .next()
            .and_then(|value| value.parse::<f64>().ok())
            .ok_or_else(|| SpatialError::MalformedMatrix {
                line: line_number,
                message: "invalid matrix value".to_owned(),
            })?;
        if row < 1 || column < 1 {
            return Err(SpatialError::MalformedMatrix {
                line: line_number,
                message: "matrix indices are 1-based".to_owned(),
            });
        }
        let (rows, columns) = dimensions.expect("dimensions parsed");
        if row > rows || column > columns {
            return Err(SpatialError::MalformedMatrix {
                line: line_number,
                message: "matrix index out of bounds".to_owned(),
            });
        }
        let barcode_index = (column - 1) as usize;
        result.counts_per_barcode[barcode_index] += value.round() as u64;
        if value > 0.0 {
            result.genes_per_barcode[barcode_index] += 1;
            result.n_nonzero += 1;
            result.total_counts += value.round() as u64;
        }
    }
    if !header_seen {
        return Err(SpatialError::MalformedMatrix {
            line: 1,
            message: "missing matrix market header and dimensions".to_owned(),
        });
    }
    if count_entries > 0 && result.n_nonzero > count_entries {
        return Err(SpatialError::MalformedMatrix {
            line: 1,
            message: "more entries than declared in the matrix header".to_owned(),
        });
    }

    let mut counts = result.counts_per_barcode.clone();
    counts.sort_unstable();
    result.mean_counts = if result.n_barcodes > 0 {
        Some(result.total_counts as f64 / result.n_barcodes as f64)
    } else {
        None
    };
    let genes = {
        let mut sorted = result.genes_per_barcode.clone();
        sorted.sort_unstable();
        sorted
    };
    result.median_genes = percentile(&genes, 0.5);
    result.p90_genes = percentile(&genes, 0.9);
    result.barcode_rank = {
        let mut ranked: Vec<(u64, String)> = barcodes
            .iter()
            .take(result.n_barcodes as usize)
            .cloned()
            .enumerate()
            .map(|(index, barcode)| {
                (
                    result.counts_per_barcode[index],
                    barcode.trim_end_matches('\n').to_owned(),
                )
            })
            .collect();
        ranked.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        ranked
            .into_iter()
            .enumerate()
            .map(|(rank, (total_counts, barcode))| {
                let barcode_index = result
                    .counts_per_barcode
                    .iter()
                    .position(|count| *count == total_counts)
                    .unwrap_or(0);
                BarcodeRank {
                    rank: (rank + 1) as u64,
                    barcode,
                    total_counts,
                    n_genes: result.genes_per_barcode[barcode_index],
                }
            })
            .collect()
    };
    if result.n_nonzero == 0 {
        result
            .warnings
            .push("matrix contains no nonzero entries".to_owned());
    }
    Ok(result)
}

fn percentile(sorted: &[u64], fraction: f64) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    Some(sorted[index])
}

/// Render the barcode rank table (rank, barcode, total_counts, n_genes).
pub fn render_barcode_rank_table(result: &SpatialTranscriptomicsResult) -> String {
    let mut table = String::from("rank\tbarcode\ttotal_counts\tn_genes\n");
    for entry in &result.barcode_rank {
        table.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            entry.rank, entry.barcode, entry.total_counts, entry.n_genes
        ));
    }
    table
}
// Convenience alias used by tests and callers that prefer unqualified names.
pub fn spatial_transcriptomics(
    matrix_path: impl AsRef<Path>,
    features_path: impl AsRef<Path>,
    barcodes_path: impl AsRef<Path>,
) -> Result<SpatialTranscriptomicsResult, SpatialError> {
    spatial_transcriptomics_path(matrix_path, features_path, barcodes_path)
}

#[cfg(test)]
mod tests {
    use super::{render_barcode_rank_table, spatial_transcriptomics};
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TICK: AtomicU64 = AtomicU64::new(0);

    // test helper reading from in-memory strings via temp files
    fn analyze(
        matrix: &str,
        features: &str,
        barcodes: &str,
    ) -> Result<super::SpatialTranscriptomicsResult, super::SpatialError> {
        let dir = std::env::temp_dir().join(format!(
            "linxira-spatial-test-{}-{}",
            std::process::id(),
            TICK.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let matrix_path = dir.join("matrix.mtx");
        let features_path = dir.join("features.tsv");
        let barcodes_path = dir.join("barcodes.tsv");
        std::fs::write(&matrix_path, matrix).expect("matrix");
        std::fs::write(&features_path, features).expect("features");
        std::fs::write(&barcodes_path, barcodes).expect("barcodes");
        let result = spatial_transcriptomics(&matrix_path, &features_path, &barcodes_path);
        std::fs::remove_dir_all(&dir).expect("cleanup");
        result
    }

    #[test]
    fn parses_a_10x_matrix_and_computes_barcode_statistics() {
        let result = analyze(
            "%%MatrixMarket matrix coordinate real general\n%comment\n3 2 4\n1 1 5\n2 1 3\n3 1 0.5\n3 2 7\n",
            "gene1\tENSMUSG1\ngene2\tENSMUSG2\ngene3\tENSMUSG3\n",
            "AAACCT\nAAAGGT\n",
        )
        .expect("parse 10x matrix");
        assert_eq!(result.n_features, 3);
        assert_eq!(result.n_barcodes, 2);
        assert_eq!(result.n_nonzero, 4);
        assert_eq!(result.total_counts, 16); // entries are rounded individually (0.5 -> 1)
        assert_eq!(result.counts_per_barcode, vec![9, 7]);
        assert_eq!(result.genes_per_barcode, vec![3, 1]);
        assert_eq!(result.median_genes, Some(3)); // upper median of [1, 3]
        assert_eq!(result.barcode_rank[0].rank, 1);
        assert_eq!(result.barcode_rank[0].barcode, "AAACCT");
        assert_eq!(result.barcode_rank[0].total_counts, 9);
        let table = render_barcode_rank_table(&result);
        assert!(table.starts_with("rank\tbarcode\ttotal_counts\tn_genes\n"));
        assert!(table.contains("AAACCT\t9\t3"));
    }

    #[test]
    fn rejects_mismatched_annotations() {
        let error = analyze(
            "%%MatrixMarket matrix coordinate real general\n3 2 1\n1 1 1\n",
            "gene1\ngene2\ngene3\n",
            "only-one-barcode\n",
        )
        .expect_err("dimension mismatch must fail");
        assert!(error.to_string().contains("do not match annotations"));
    }

    #[test]
    fn supports_gzip_compressed_matrices() {
        let dir = std::env::temp_dir().join(format!("linxira-spatial-gz-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let matrix_path = dir.join("matrix.mtx.gz");
        let features_path = dir.join("features.tsv.gz");
        let barcodes_path = dir.join("barcodes.tsv.gz");
        let compress = |text: &str| -> Vec<u8> {
            use flate2::Compression;
            use flate2::write::GzEncoder;
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(text.as_bytes()).expect("compress");
            encoder.finish().expect("finish")
        };
        std::fs::write(
            &matrix_path,
            compress("%%MatrixMarket matrix coordinate real general\n2 2 2\n1 1 4\n2 2 9\n"),
        )
        .expect("matrix");
        std::fs::write(
            &features_path,
            compress("gene1\tENSMUSG1\ngene2\tENSMUSG2\n"),
        )
        .expect("features");
        std::fs::write(&barcodes_path, compress("b1\nb2\n")).expect("barcodes");
        let result = spatial_transcriptomics(&matrix_path, &features_path, &barcodes_path)
            .expect("parse gzip matrix");
        assert_eq!(result.total_counts, 13);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
