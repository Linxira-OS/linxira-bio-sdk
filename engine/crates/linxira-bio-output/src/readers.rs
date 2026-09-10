//! Traditional bioinformatics format readers (M0-T4).
//!
//! Each parser inverts its writer exactly: canonical JSON records go out, the
//! traditional file goes back into the same canonical shape. Interval starts
//! are converted back to the 1-based closed JSON convention, `.`/`*`
//! missing-value markers are dropped, and numbers are restored where the
//! writer rendered them from numbers.

use crate::{BED_COLUMNS, CoordinateSystem, OutputError, OutputResult};
use linxira_bio_protocol::BioDataFormat;
use serde_json::{Map, Value};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// The unified entry point for reading traditional artifacts back into JSON.
pub struct BioDataReader;

impl BioDataReader {
    /// Infers the format from the path extension and parses the file.
    pub fn read_path(path: &Path) -> OutputResult<Value> {
        let format = crate::format_from_path(path)?;
        Self::read(format, path)
    }

    /// Parses a file whose format is already known.
    pub fn read(format: BioDataFormat, path: &Path) -> OutputResult<Value> {
        let records = Self::read_records(format, path)?;
        Ok(Value::Array(
            records.into_iter().map(Value::Object).collect(),
        ))
    }

    /// Parses a file into canonical record objects.
    pub fn read_records(
        format: BioDataFormat,
        path: &Path,
    ) -> OutputResult<Vec<Map<String, Value>>> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        match format {
            BioDataFormat::Fasta => read_fasta(reader),
            BioDataFormat::Fastq => read_fastq(reader),
            BioDataFormat::Bed => read_bed(reader),
            BioDataFormat::Gff3 => read_gff3(reader),
            BioDataFormat::Gtf => read_gtf(reader),
            BioDataFormat::Vcf => read_vcf(reader),
            BioDataFormat::Sam => read_sam(reader),
            other => Err(OutputError::UnsupportedFormat(format!(
                "{other:?} has no record reader; table formats flow through the export crate"
            ))),
        }
    }
}

/// Line wrapper that keeps 1-based line numbers for error messages and
/// normalizes CRLF endings.
struct LineReader<R: BufRead> {
    inner: R,
    number: usize,
}

impl<R: BufRead> LineReader<R> {
    fn new(inner: R) -> Self {
        Self { inner, number: 0 }
    }

    fn next_line(&mut self) -> OutputResult<Option<String>> {
        let mut line = String::new();
        let bytes = self.inner.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        self.number += 1;
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        Ok(Some(line))
    }

    fn invalid(&self, message: impl std::fmt::Display) -> OutputError {
        OutputError::InvalidRecord(format!("line {}: {message}", self.number))
    }
}

fn read_fasta<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut records: Vec<Map<String, Value>> = Vec::new();
    let mut current: Option<(String, Option<String>, String)> = None;
    while let Some(line) = lines.next_line()? {
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('>') {
            if let Some((id, description, sequence)) = current.take() {
                records.push(fasta_record(id, description, sequence));
            }
            let header = header.trim_start();
            let (id, description) = match header.find(char::is_whitespace) {
                Some(split) => {
                    let (id, rest) = header.split_at(split);
                    let description = rest.trim();
                    (
                        id.to_owned(),
                        if description.is_empty() {
                            None
                        } else {
                            Some(description.to_owned())
                        },
                    )
                }
                None => (header.to_owned(), None),
            };
            if id.is_empty() {
                return Err(lines.invalid("FASTA header has no identifier"));
            }
            current = Some((id, description, String::new()));
        } else {
            let sequence = match &mut current {
                Some((_, _, sequence)) => sequence,
                None => {
                    return Err(lines.invalid("sequence data appears before a FASTA header"));
                }
            };
            sequence.push_str(line.trim());
        }
    }
    if let Some((id, description, sequence)) = current.take() {
        records.push(fasta_record(id, description, sequence));
    }
    Ok(records)
}

fn fasta_record(id: String, description: Option<String>, sequence: String) -> Map<String, Value> {
    let mut record = Map::new();
    record.insert("id".to_owned(), Value::String(id));
    if let Some(description) = description {
        record.insert("description".to_owned(), Value::String(description));
    }
    record.insert("sequence".to_owned(), Value::String(sequence));
    record
}

fn read_fastq<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut records = Vec::new();
    while let Some(header) = lines.next_line()? {
        if header.is_empty() {
            continue;
        }
        let header = header.strip_prefix('@').ok_or_else(|| {
            lines.invalid(format!(
                "expected a FASTQ header starting with '@', got {header:?}"
            ))
        })?;
        let (id, description) = split_optional_description(header);
        let sequence = lines
            .next_line()?
            .ok_or_else(|| lines.invalid("the file ends before the FASTQ sequence line"))?;
        let plus = lines
            .next_line()?
            .ok_or_else(|| lines.invalid("the file ends before the FASTQ separator line"))?;
        if !plus.starts_with('+') {
            return Err(lines.invalid(format!(
                "expected the FASTQ separator '+' line, got {plus:?}"
            )));
        }
        let quality = lines
            .next_line()?
            .ok_or_else(|| lines.invalid("the file ends before the FASTQ quality line"))?;
        if quality.chars().count() != sequence.chars().count() {
            return Err(lines.invalid(format!(
                "FASTQ sequence length {} does not match quality length {}",
                sequence.chars().count(),
                quality.chars().count()
            )));
        }
        let mut record = Map::new();
        record.insert("id".to_owned(), Value::String(id));
        if let Some(description) = description {
            record.insert("description".to_owned(), Value::String(description));
        }
        record.insert("sequence".to_owned(), Value::String(sequence));
        record.insert("quality".to_owned(), Value::String(quality));
        records.push(record);
    }
    Ok(records)
}

fn split_optional_description(header: &str) -> (String, Option<String>) {
    match header.find(char::is_whitespace) {
        Some(split) => {
            let (id, rest) = header.split_at(split);
            let description = rest.trim();
            (
                id.to_owned(),
                if description.is_empty() {
                    None
                } else {
                    Some(description.to_owned())
                },
            )
        }
        None => (header.to_owned(), None),
    }
}

fn read_bed<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut records = Vec::new();
    while let Some(line) = lines.next_line()? {
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("track ")
            || line.starts_with("browser ")
        {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            return Err(lines.invalid(format!(
                "a BED line needs at least 3 columns, got {}",
                fields.len()
            )));
        }
        if fields.len() > BED_COLUMNS.len() {
            return Err(lines.invalid(format!(
                "a BED line supports at most {} columns, got {}",
                BED_COLUMNS.len(),
                fields.len()
            )));
        }
        let start: u64 = fields[1]
            .parse()
            .map_err(|_| lines.invalid(format!("BED start {:?} is not an integer", fields[1])))?;
        let end: u64 = fields[2]
            .parse()
            .map_err(|_| lines.invalid(format!("BED end {:?} is not an integer", fields[2])))?;
        let mut record = Map::new();
        record.insert("chrom".to_owned(), Value::String(fields[0].to_owned()));
        record.insert(
            "start".to_owned(),
            Value::from(CoordinateSystem::Bed.json_start(start)),
        );
        record.insert("end".to_owned(), Value::from(end));
        for (column, field) in BED_COLUMNS[3..].iter().zip(fields[3..].iter()) {
            if field.is_empty() {
                continue;
            }
            // Skip the canonical fill values the writer pads rows with, so a
            // padded file reads back to the same canonical records.
            if *field == bed_default_token(column) {
                continue;
            }
            let value = match *column {
                "score" | "thickStart" | "thickEnd" | "blockCount" => json_number_or_string(field),
                _ => Value::String((*field).to_owned()),
            };
            record.insert((*column).to_owned(), value);
        }
        records.push(record);
    }
    Ok(records)
}

fn read_gff3<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut records = Vec::new();
    while let Some(line) = lines.next_line()? {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 9 {
            return Err(lines.invalid(format!(
                "a GFF3 line needs exactly 9 columns, got {}",
                fields.len()
            )));
        }
        let start: u64 = fields[3]
            .parse()
            .map_err(|_| lines.invalid(format!("GFF3 start {:?} is not an integer", fields[3])))?;
        let end: u64 = fields[4]
            .parse()
            .map_err(|_| lines.invalid(format!("GFF3 end {:?} is not an integer", fields[4])))?;
        let mut record = Map::new();
        record.insert("seqid".to_owned(), Value::String(fields[0].to_owned()));
        insert_optional_text(&mut record, "source", fields[1]);
        insert_optional_text(&mut record, "type", fields[2]);
        record.insert(
            "start".to_owned(),
            Value::from(CoordinateSystem::Gff.json_start(start)),
        );
        record.insert("end".to_owned(), Value::from(end));
        insert_optional_number(&mut record, "score", fields[5]);
        insert_optional_text(&mut record, "strand", fields[6]);
        insert_optional_phase(&mut record, "phase", fields[7]);
        if fields[8] != "." {
            record.insert(
                "attributes".to_owned(),
                Value::Object(parse_gff3_attributes(fields[8])),
            );
        }
        records.push(record);
    }
    Ok(records)
}

fn parse_gff3_attributes(text: &str) -> Map<String, Value> {
    let mut attributes = Map::new();
    for part in text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('=') {
            Some((key, value)) => {
                attributes.insert(
                    percent_decode(key.trim()),
                    Value::String(percent_decode(value)),
                );
            }
            None => {
                attributes.insert(percent_decode(part), Value::Bool(true));
            }
        }
    }
    attributes
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 3 <= bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_gtf<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut records = Vec::new();
    while let Some(line) = lines.next_line()? {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 9 {
            return Err(lines.invalid(format!(
                "a GTF line needs exactly 9 columns, got {}",
                fields.len()
            )));
        }
        let start: u64 = fields[3]
            .parse()
            .map_err(|_| lines.invalid(format!("GTF start {:?} is not an integer", fields[3])))?;
        let end: u64 = fields[4]
            .parse()
            .map_err(|_| lines.invalid(format!("GTF end {:?} is not an integer", fields[4])))?;
        let mut record = Map::new();
        record.insert("seqid".to_owned(), Value::String(fields[0].to_owned()));
        insert_optional_text(&mut record, "source", fields[1]);
        insert_optional_text(&mut record, "type", fields[2]);
        record.insert(
            "start".to_owned(),
            Value::from(CoordinateSystem::Gff.json_start(start)),
        );
        record.insert("end".to_owned(), Value::from(end));
        insert_optional_number(&mut record, "score", fields[5]);
        insert_optional_text(&mut record, "strand", fields[6]);
        insert_optional_phase(&mut record, "frame", fields[7]);
        if fields[8] != "." {
            record.insert(
                "attributes".to_owned(),
                Value::Object(parse_gtf_attributes(fields[8])),
            );
        }
        records.push(record);
    }
    Ok(records)
}

fn parse_gtf_attributes(text: &str) -> Map<String, Value> {
    let mut attributes = Map::new();
    for part in text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((key, raw)) = part.split_once(' ') else {
            attributes.insert(part.to_owned(), Value::Bool(true));
            continue;
        };
        let raw = raw.trim();
        let value = if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
            Value::String(unescape_gtf(&raw[1..raw.len() - 1]))
        } else if raw == "true" || raw == "false" {
            Value::Bool(raw == "true")
        } else {
            json_number_or_string(raw)
        };
        attributes.insert(key.to_owned(), value);
    }
    attributes
}

fn unescape_gtf(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            match characters.next() {
                Some(escaped) => out.push(escaped),
                None => out.push('\\'),
            }
        } else {
            out.push(character);
        }
    }
    out
}

fn read_vcf<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut sample_names: Vec<String> = Vec::new();
    let mut has_format_column = false;
    let mut records = Vec::new();
    while let Some(line) = lines.next_line()? {
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix("#CHROM") {
            let columns: Vec<&str> = header.split('\t').collect();
            has_format_column = columns.len() > 8 && columns[8] == "FORMAT";
            if columns.len() > 9 {
                sample_names = columns[9..].iter().map(|name| (*name).to_owned()).collect();
            } else {
                sample_names = Vec::new();
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 8 {
            return Err(lines.invalid(format!(
                "a VCF line needs at least 8 columns, got {}",
                fields.len()
            )));
        }
        let pos: u64 = fields[1]
            .parse()
            .map_err(|_| lines.invalid(format!("VCF POS {:?} is not an integer", fields[1])))?;
        let mut record = Map::new();
        record.insert("chrom".to_owned(), Value::String(fields[0].to_owned()));
        record.insert(
            "pos".to_owned(),
            Value::from(CoordinateSystem::Vcf.json_start(pos)),
        );
        insert_optional_text(&mut record, "id", fields[2]);
        record.insert("ref".to_owned(), Value::String(fields[3].to_owned()));
        insert_optional_text(&mut record, "alt", fields[4]);
        insert_optional_number(&mut record, "qual", fields[5]);
        if fields[6] != "." {
            if fields[6].contains(';') {
                record.insert(
                    "filter".to_owned(),
                    Value::Array(
                        fields[6]
                            .split(';')
                            .map(|filter| Value::String(filter.to_owned()))
                            .collect(),
                    ),
                );
            } else {
                record.insert("filter".to_owned(), Value::String(fields[6].to_owned()));
            }
        }
        if fields[7] != "." {
            record.insert("info".to_owned(), Value::Object(parse_vcf_info(fields[7])));
        }
        if has_format_column {
            if fields.len() < 9 {
                return Err(lines
                    .invalid("the VCF header declares a FORMAT column but the record has none"));
            }
            insert_optional_text(&mut record, "format", fields[8]);
            let samples = &fields[9..];
            if samples.len() != sample_names.len() {
                return Err(lines.invalid(format!(
                    "the VCF header declares {} sample columns but the record carries {}",
                    sample_names.len(),
                    samples.len()
                )));
            }
            record.insert(
                "samples".to_owned(),
                Value::Array(
                    samples
                        .iter()
                        .map(|sample| Value::String((*sample).to_owned()))
                        .collect(),
                ),
            );
        }
        records.push(record);
    }
    Ok(records)
}

fn parse_vcf_info(text: &str) -> Map<String, Value> {
    let mut info = Map::new();
    for part in text.split(';') {
        if part.is_empty() {
            continue;
        }
        match part.split_once('=') {
            Some((key, value)) => {
                let parsed = if value.contains(',') {
                    Value::Array(
                        value
                            .split(',')
                            .map(json_number_or_string)
                            .collect::<Vec<_>>(),
                    )
                } else {
                    json_number_or_string(value)
                };
                info.insert(key.to_owned(), parsed);
            }
            None => {
                info.insert(part.to_owned(), Value::Bool(true));
            }
        }
    }
    info
}

fn read_sam<R: BufRead>(reader: R) -> OutputResult<Vec<Map<String, Value>>> {
    let mut lines = LineReader::new(reader);
    let mut records = Vec::new();
    while let Some(line) = lines.next_line()? {
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 11 {
            return Err(lines.invalid(format!(
                "a SAM line needs at least 11 columns, got {}",
                fields.len()
            )));
        }
        let flag: u64 = fields[1]
            .parse()
            .map_err(|_| lines.invalid(format!("SAM FLAG {:?} is not an integer", fields[1])))?;
        let pos: u64 = fields[3]
            .parse()
            .map_err(|_| lines.invalid(format!("SAM POS {:?} is not an integer", fields[3])))?;
        let mapq: u64 = fields[4]
            .parse()
            .map_err(|_| lines.invalid(format!("SAM MAPQ {:?} is not an integer", fields[4])))?;
        let pnext: u64 = fields[7]
            .parse()
            .map_err(|_| lines.invalid(format!("SAM PNEXT {:?} is not an integer", fields[7])))?;
        let tlen: i64 = fields[8]
            .parse()
            .map_err(|_| lines.invalid(format!("SAM TLEN {:?} is not an integer", fields[8])))?;
        let mut record = Map::new();
        record.insert("qname".to_owned(), Value::String(fields[0].to_owned()));
        record.insert("flag".to_owned(), Value::from(flag));
        insert_optional_text(&mut record, "rname", fields[2]);
        record.insert("pos".to_owned(), Value::from(pos));
        record.insert("mapq".to_owned(), Value::from(mapq));
        insert_optional_text(&mut record, "cigar", fields[5]);
        insert_optional_text(&mut record, "rnext", fields[6]);
        record.insert("pnext".to_owned(), Value::from(pnext));
        record.insert("tlen".to_owned(), Value::from(tlen));
        record.insert("seq".to_owned(), Value::String(fields[9].to_owned()));
        record.insert("qual".to_owned(), Value::String(fields[10].to_owned()));
        if fields.len() > 11 {
            record.insert(
                "tags".to_owned(),
                Value::Array(
                    fields[11..]
                        .iter()
                        .map(|tag| Value::String((*tag).to_owned()))
                        .collect(),
                ),
            );
        }
        records.push(record);
    }
    Ok(records)
}

/// The fill value the writer pads this BED column with; such tokens are
/// omitted on read so padded files stay reversible.
fn bed_default_token(column: &str) -> &'static str {
    match column {
        "name" | "strand" => ".",
        _ => "0",
    }
}

fn insert_optional_text(record: &mut Map<String, Value>, key: &str, field: &str) {
    if !field.is_empty() && field != "." && field != "*" {
        record.insert(key.to_owned(), Value::String(field.to_owned()));
    }
}

fn insert_optional_number(record: &mut Map<String, Value>, key: &str, field: &str) {
    if !field.is_empty() && field != "." {
        record.insert(key.to_owned(), json_number_or_string(field));
    }
}

fn insert_optional_phase(record: &mut Map<String, Value>, key: &str, field: &str) {
    if !field.is_empty() && field != "." {
        record.insert(key.to_owned(), json_number_or_string(field));
    }
}

/// Parses numeric-looking text back into JSON numbers so the reader inverts
/// the writer's number rendering; anything else stays a string.
fn json_number_or_string(text: &str) -> Value {
    if let Ok(integer) = text.parse::<i64>() {
        return Value::from(integer);
    }
    if let Ok(unsigned) = text.parse::<u64>() {
        return Value::from(unsigned);
    }
    if let Ok(float) = text.parse::<f64>()
        && let Some(number) = serde_json::Number::from_f64(float)
    {
        return Value::Number(number);
    }
    Value::String(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{json_number_or_string, percent_decode, read_fasta, read_fastq};
    use crate::BioDataReader;
    use linxira_bio_protocol::BioDataFormat;
    use serde_json::json;
    use std::io::BufReader;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "linxira-bio-output-readers-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temporary root");
        root
    }

    #[test]
    fn parses_fasta_with_crlf_wrapping_and_descriptions() {
        let text = ">seq1 desc with spaces\r\nACGT\r\nAC\r\n>seq2\r\nGG\r\n";
        let records = read_fasta(BufReader::new(text.as_bytes())).expect("parse FASTA");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["id"], json!("seq1"));
        assert_eq!(records[0]["description"], json!("desc with spaces"));
        assert_eq!(records[0]["sequence"], json!("ACGTAC"));
        assert!(records[1].get("description").is_none());
    }

    #[test]
    fn rejects_fasta_sequence_before_header() {
        let error = read_fasta(BufReader::new("ACGT\n>s\nAC\n".as_bytes()))
            .expect_err("sequence before header must fail");
        assert!(error.to_string().contains("before a FASTA header"));
    }

    #[test]
    fn fastq_errors_carry_line_numbers() {
        let text = "@r1\nACGT\n-\nIIII\n";
        let error =
            read_fastq(BufReader::new(text.as_bytes())).expect_err("missing separator must fail");
        assert!(error.to_string().contains("line 3"));

        let truncated = "@r1\nACGT\n";
        let error = read_fastq(BufReader::new(truncated.as_bytes()))
            .expect_err("truncated record must fail");
        assert!(error.to_string().contains("separator"));
    }

    #[test]
    fn reads_bed_files_written_by_third_parties() {
        let root = temp_root("bed-foreign");
        let path = root.join("external.bed");
        std::fs::write(
            &path,
            "track name=\"external\"\nchr2\t100\t200\tpeak\t999\t-\t100\t200\t255,0,0\n",
        )
        .expect("write external BED");
        let value = BioDataReader::read(BioDataFormat::Bed, &path).expect("read BED");
        assert_eq!(value[0]["chrom"], json!("chr2"));
        assert_eq!(value[0]["start"], json!(101));
        assert_eq!(value[0]["end"], json!(200));
        assert_eq!(value[0]["itemRgb"], json!("255,0,0"));
        assert_eq!(value[0]["score"], json!(999));
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn decodes_percent_escapes_and_vcf_info_values() {
        assert_eq!(percent_decode("brca%3B1%25x"), "brca;1%x");
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[test]
    fn parses_numeric_looking_text_back_into_numbers() {
        assert_eq!(json_number_or_string("42"), json!(42));
        assert_eq!(json_number_or_string("0.5"), json!(0.5));
        assert_eq!(json_number_or_string("nan"), json!("nan"));
    }

    #[test]
    fn unknown_formats_are_rejected() {
        let root = temp_root("unsupported");
        let path = root.join("tree.newick");
        std::fs::write(&path, "(a,b);").expect("write newick");
        assert!(BioDataReader::read_path(&path).is_err());
        std::fs::remove_dir_all(root).expect("clean up");
    }
}
