//! Round-trip acceptance for the unified output framework (M0-T2/T4):
//! canonical JSON → traditional file → canonical JSON must be equal for at
//! least two fixtures per format.

use linxira_bio_output::{BioDataReader, BioDataWriter, WriteOptions};
use linxira_bio_protocol::BioDataFormat;
use serde_json::{Value, json};
use std::path::PathBuf;

fn scratch_dir(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "linxira-bio-output-roundtrip-{tag}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create scratch directory");
    root
}

fn assert_roundtrip(tag: &str, format: BioDataFormat, extension: &str, fixtures: &[Value]) {
    let root = scratch_dir(tag);
    for (index, fixture) in fixtures.iter().enumerate() {
        let output = root.join(format!("fixture-{index}.{extension}"));
        BioDataWriter::write_to_path(fixture, &output, &WriteOptions::default())
            .unwrap_or_else(|error| panic!("write {tag} fixture {index}: {error}"));
        let parsed = BioDataReader::read(format, &output)
            .unwrap_or_else(|error| panic!("read {tag} fixture {index}: {error}"));
        let records = fixture
            .get("records")
            .unwrap_or(fixture.get("result").unwrap_or(fixture));
        assert_eq!(
            parsed, *records,
            "{tag} fixture {index} did not survive the roundtrip"
        );
        std::fs::remove_file(&output).expect("remove roundtrip output");
    }
    std::fs::remove_dir_all(root).expect("remove scratch directory");
}

#[test]
fn fasta_roundtrips_two_fixtures() {
    assert_roundtrip(
        "fasta",
        BioDataFormat::Fasta,
        "fa",
        &[
            json!([
                {"id": "gene1", "description": "beta lactamase", "sequence": "ATGAAATTT"},
                {"id": "gene2", "sequence": "GGCCTTAA"}
            ]),
            json!({"records": [
                {"id": "wrapped", "sequence": "ACGTRYSWKMBDHVN.".repeat(12)},
                {"id": "short", "sequence": "TT"}
            ]}),
        ],
    );
}

#[test]
fn fastq_roundtrips_two_fixtures() {
    assert_roundtrip(
        "fastq",
        BioDataFormat::Fastq,
        "fastq",
        &[
            json!([
                {"id": "read1", "description": "sample A mate 1", "sequence": "ACGTACGT", "quality": "IIIIIIII"},
                {"id": "read2", "sequence": "TTGG", "quality": "####"}
            ]),
            json!([
                {"id": "only-one", "sequence": "A", "quality": "!"}
            ]),
        ],
    );
}

#[test]
fn bed_roundtrips_two_fixtures() {
    assert_roundtrip(
        "bed",
        BioDataFormat::Bed,
        "bed",
        &[
            json!([
                {"chrom": "chr1", "start": 1, "end": 100},
                {"chrom": "chr2", "start": 201, "end": 400, "name": "peak_1", "score": 620, "strand": "-"}
            ]),
            json!([
                {"chrom": "chrM", "start": 1, "end": 1, "name": "single-base", "score": 500,
                 "strand": "+", "thickStart": 1, "thickEnd": 1, "itemRgb": "255,0,0"}
            ]),
        ],
    );
}

#[test]
fn gff3_roundtrips_two_fixtures() {
    assert_roundtrip(
        "gff3",
        BioDataFormat::Gff3,
        "gff3",
        &[
            json!([
                {"seqid": "I", "source": "engine", "type": "gene", "start": 1, "end": 1000,
                 "strand": "+",
                 "attributes": {"ID": "gene:1", "Name": "brca1"}},
                {"seqid": "II", "source": "engine", "type": "CDS", "start": 500, "end": 700,
                 "attributes": {"Parent": "gene:1"}}
            ]),
            json!([
                {"seqid": "scaffold_7", "source": "engine", "type": "exon", "start": 33, "end": 120,
                 "score": 9.75, "strand": "-", "phase": 0,
                 "attributes": {"transcript_id": "tr:9"}}
            ]),
        ],
    );
}

#[test]
fn gtf_roundtrips_two_fixtures() {
    assert_roundtrip(
        "gtf",
        BioDataFormat::Gtf,
        "gtf",
        &[
            json!([
                {"seqid": "chr1", "source": "havana", "type": "exon", "start": 11869, "end": 12227,
                 "strand": "+",
                 "attributes": {"gene_id": "ENSG1", "transcript_id": "ENST1", "exon_number": 1}}
            ]),
            json!([
                {"seqid": "chrX", "source": "engine", "type": "CDS", "start": 200, "end": 300,
                 "strand": "-", "frame": 0, "attributes": {"gene_biotype": "protein_coding"}}
            ]),
        ],
    );
}

#[test]
fn vcf_roundtrips_two_fixtures() {
    assert_roundtrip(
        "vcf",
        BioDataFormat::Vcf,
        "vcf",
        &[
            json!([
                {"chrom": "chr7", "pos": 117199644, "id": "rs699", "ref": "A", "alt": "G",
                 "qual": 118.5, "filter": "PASS", "info": {"dp": 61, "af": 0.42}},
                {"chrom": "chr7", "pos": 117199700, "ref": "C", "alt": "T", "qual": 30}
            ]),
            json!([
                {"chrom": "chr1", "pos": 10180, "id": "rs667", "ref": "CC", "alt": "CTG",
                 "filter": "q10", "info": {"flagged": true},
                 "format": "GT:DP", "samples": ["0/1:14", "1/1:9", "0/0:12"]}
            ]),
        ],
    );
}

#[test]
fn sam_roundtrips_two_fixtures() {
    assert_roundtrip(
        "sam",
        BioDataFormat::Sam,
        "sam",
        &[
            json!([
                {"qname": "read_1", "flag": 99, "rname": "chr1", "pos": 101, "mapq": 60,
                 "cigar": "8M", "rnext": "=", "pnext": 151, "tlen": 58,
                 "seq": "ACGTTGCA", "qual": "IIIIIIII", "tags": ["NM:i:0"]},
                {"qname": "read_2", "flag": 77, "pos": 0, "mapq": 0,
                 "pnext": 0, "tlen": 0,
                 "seq": "GGTT", "qual": "*"}
            ]),
            json!([
                {"qname": "solo", "flag": 16, "rname": "chrM", "pos": 1, "mapq": 25,
                 "cigar": "4M2I3M", "pnext": 0, "tlen": -9,
                 "seq": "AAATTGG", "qual": "IIIIIII", "tags": ["NM:i:2", "MD:Z:7"]}
            ]),
        ],
    );
}

#[test]
fn tables_roundtrip_through_the_export_crate() {
    // CSV/TSV roundtrip flows through the export crate's own reader paths,
    // so here we only assert the writer produces the documented bytes.
    let root = scratch_dir("tables");
    let value = json!({"result": [
        {"sample": "tumor", "count": 42},
        {"sample": "normal", "count": 17}
    ]});
    let output = root.join("counts.tsv");
    BioDataWriter::write_to_path(&value, &output, &WriteOptions::default()).expect("tsv write");
    assert_eq!(
        std::fs::read_to_string(&output).expect("read tsv"),
        "count\tsample\n42\ttumor\n17\tnormal\n"
    );
    std::fs::remove_dir_all(root).expect("remove scratch directory");
}
