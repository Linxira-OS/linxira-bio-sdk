//! Golden-byte alignment for the traditional-format writers (M0 §5.2).
//!
//! The artifacts under `tests/fixtures/output/<format>/` were validated with
//! the native reference tools (bedtools 2.31.1, bcftools 1.24, samtools 1.24,
//! gffread 0.12.9 on Arch Linux): bedtools sort/intersect, gffread, and
//! samtools consume them without errors and `bcftools view` without a single
//! warning. These tests pin the writer bytes to exactly those files so the
//! tool-level alignment cannot regress silently.

use linxira_bio_output::{BioDataWriter, WriteOptions};
use serde_json::Value;
use std::path::Path;

fn fixture_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/output")
}

#[test]
fn golden_artifacts_reproduce_the_reference_validated_bytes() {
    let cases = [
        ("bed", "intervals.bed"),
        ("gff3", "genes.gff3"),
        ("vcf", "variants.vcf"),
        ("sam", "alignments.sam"),
    ];
    for (directory, artifact) in cases {
        let root = fixture_root().join(directory);
        let input: Value = serde_json::from_reader(
            std::fs::File::open(root.join(format!("{artifact}.json")))
                .unwrap_or_else(|error| panic!("open {directory} input: {error}")),
        )
        .unwrap_or_else(|error| panic!("read {directory} input: {error}"));
        let golden = std::fs::read(root.join(artifact))
            .unwrap_or_else(|error| panic!("read {directory} golden: {error}"));

        let extension = artifact.rsplit('.').next().unwrap_or("txt");
        let output = std::env::temp_dir().join(format!(
            "linxira-bio-output-golden-{directory}-{}.{extension}",
            std::process::id()
        ));
        BioDataWriter::write_to_path(&input, &output, &WriteOptions::default())
            .unwrap_or_else(|error| panic!("write {directory}: {error}"));
        let rendered = std::fs::read(&output).expect("read rendered artifact");
        std::fs::remove_file(&output).expect("remove rendered artifact");

        assert_eq!(
            rendered, golden,
            "{directory} writer drifted from the reference-validated bytes"
        );
    }
}
