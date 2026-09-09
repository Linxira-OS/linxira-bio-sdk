//! Traditional bioinformatics format writers (M0-T2/T3).
//!
//! Every writer consumes canonical JSON records (see the crate docs), renders
//! bytes, and persists them atomically through
//! `linxira_bio_export::write_atomic_bytes`, so a failed write never replaces
//! an existing artifact with partial output.

use crate::{
    ArtifactSpec, BED_COLUMNS, CoordinateSystem, OutputError, OutputResult, RoleValues,
    WriteOptions, WriteReceipt, format_from_path, records_from_json, table_from_records,
};
use linxira_bio_export::{export_table, export_value, write_atomic_bytes};
use linxira_bio_protocol::BioDataFormat;
use serde_json::{Map, Value};
use std::path::Path;

/// The conventional FASTA wrap width.
pub const DEFAULT_FASTA_LINE_WIDTH: usize = 60;

/// The unified entry point for writing artifacts.
pub struct BioDataWriter;

impl BioDataWriter {
    /// Writes every artifact declared by `spec` under `workspace`. `values`
    /// maps each artifact role to the JSON document backing that artifact.
    pub fn write_spec(
        spec: &crate::OutputSpec,
        values: &RoleValues,
        workspace: &Path,
    ) -> OutputResult<Vec<WriteReceipt>> {
        spec.validate()?;
        let mut receipts = Vec::with_capacity(spec.artifacts.len());
        for artifact in &spec.artifacts {
            let value = values.get(&artifact.role).ok_or_else(|| {
                OutputError::InvalidSpec(format!(
                    "no JSON value was provided for artifact role {:?}",
                    artifact.role
                ))
            })?;
            receipts.push(Self::write_artifact(artifact, value, workspace)?);
        }
        Ok(receipts)
    }

    /// Writes a single declared artifact relative to `workspace`.
    pub fn write_artifact(
        artifact: &ArtifactSpec,
        value: &Value,
        workspace: &Path,
    ) -> OutputResult<WriteReceipt> {
        artifact_path_guard(artifact)?;
        let output = workspace.join(&artifact.path);
        create_parent_directory(&output)?;
        let inferred = format_from_path(&output)?;
        if inferred != artifact.format {
            return Err(OutputError::InvalidSpec(format!(
                "artifact role {:?} declares format {:?} but its path extension implies {:?}",
                artifact.role, artifact.format, inferred
            )));
        }
        write_format(
            artifact.format,
            value,
            &output,
            &WriteOptions::from_artifact(artifact),
            artifact.role.clone(),
        )
    }

    /// Writes one JSON document to an explicit output path, inferring the
    /// format from the file extension (the CLI `export bio` path).
    pub fn write_to_path(
        value: &Value,
        output: &Path,
        options: &WriteOptions,
    ) -> OutputResult<WriteReceipt> {
        create_parent_directory(output)?;
        let format = format_from_path(output)?;
        write_format(format, value, output, options, "export".to_owned())
    }
}

fn artifact_path_guard(artifact: &ArtifactSpec) -> OutputResult<()> {
    let path = Path::new(&artifact.path);
    if path.is_absolute() || path.has_root() {
        return Err(OutputError::InvalidSpec(format!(
            "artifact path {:?} must be relative to the workspace output directory",
            artifact.path
        )));
    }
    Ok(())
}

fn create_parent_directory(output: &Path) -> OutputResult<()> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn write_format(
    format: BioDataFormat,
    value: &Value,
    output: &Path,
    options: &WriteOptions,
    role: String,
) -> OutputResult<WriteReceipt> {
    let records = records_from_json(value)?;
    let target = options
        .coords
        .unwrap_or(CoordinateSystem::for_format(format));
    match format {
        BioDataFormat::Fasta => {
            persist_bytes(fasta_bytes(&records, options)?, output, role, format)
        }
        BioDataFormat::Fastq => persist_bytes(fastq_bytes(&records)?, output, role, format),
        BioDataFormat::Bed => persist_bytes(bed_bytes(&records, target)?, output, role, format),
        BioDataFormat::Gff3 => persist_bytes(gff3_bytes(&records, target)?, output, role, format),
        BioDataFormat::Gtf => persist_bytes(gtf_bytes(&records, target)?, output, role, format),
        BioDataFormat::Vcf => {
            persist_bytes(vcf_bytes(&records, target, options)?, output, role, format)
        }
        BioDataFormat::Sam => persist_bytes(sam_bytes(&records, options)?, output, role, format),
        BioDataFormat::Csv | BioDataFormat::Tsv | BioDataFormat::Xlsx => {
            let table = match &options.columns {
                Some(columns) => table_with_columns(&records, columns)?,
                None => table_from_records(&records),
            };
            let exported = export_table(&table, output)?;
            Ok(WriteReceipt {
                role,
                format,
                output_path: output.to_path_buf(),
                size_bytes: exported.size_bytes,
            })
        }
        BioDataFormat::Json | BioDataFormat::Jsonl => {
            let exported = export_value(value, output)?;
            Ok(WriteReceipt {
                role,
                format,
                output_path: output.to_path_buf(),
                size_bytes: exported.size_bytes,
            })
        }
        other => Err(OutputError::UnsupportedFormat(format!(
            "{other:?} artifacts are rendered by the plot backends (M1), not the record writers"
        ))),
    }
}

fn persist_bytes(
    bytes: Vec<u8>,
    output: &Path,
    role: String,
    format: BioDataFormat,
) -> OutputResult<WriteReceipt> {
    let size_bytes = write_atomic_bytes(output, &bytes)?;
    Ok(WriteReceipt {
        role,
        format,
        output_path: output.to_path_buf(),
        size_bytes,
    })
}

fn table_with_columns(
    records: &[Map<String, Value>],
    columns: &[String],
) -> OutputResult<linxira_bio_export::Table> {
    let rows = records
        .iter()
        .map(|record| {
            columns
                .iter()
                .map(|column| record.get(column).cloned().unwrap_or(Value::Null))
                .collect()
        })
        .collect();
    Ok(linxira_bio_export::Table {
        columns: columns.to_vec(),
        rows,
    })
}

fn fasta_bytes(records: &[Map<String, Value>], options: &WriteOptions) -> OutputResult<Vec<u8>> {
    let width = options.fasta_line_width.unwrap_or(DEFAULT_FASTA_LINE_WIDTH);
    if width == 0 {
        return Err(OutputError::InvalidRecord(
            "the FASTA line width must be at least 1".to_owned(),
        ));
    }
    let mut out = String::new();
    for (index, record) in records.iter().enumerate() {
        let id = required_str(record, "id", index)?;
        let sequence = required_str(record, "sequence", index)?;
        out.push('>');
        out.push_str(&id);
        if let Some(description) = optional_str(record, "description") {
            out.push(' ');
            out.push_str(&description);
        }
        out.push('\n');
        push_wrapped(&mut out, &sequence, width);
    }
    Ok(out.into_bytes())
}

fn push_wrapped(out: &mut String, text: &str, width: usize) {
    if text.is_empty() {
        return;
    }
    for (written, character) in text.chars().enumerate() {
        if written > 0 && written.is_multiple_of(width) {
            out.push('\n');
        }
        out.push(character);
    }
    out.push('\n');
}

fn fastq_bytes(records: &[Map<String, Value>]) -> OutputResult<Vec<u8>> {
    let mut out = String::new();
    for (index, record) in records.iter().enumerate() {
        let id = required_str(record, "id", index)?;
        let sequence = required_str(record, "sequence", index)?;
        let quality = required_str(record, "quality", index)?;
        let sequence_length = sequence.chars().count();
        let quality_length = quality.chars().count();
        if sequence_length != quality_length {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: FASTQ sequence length {sequence_length} does not match \
                 quality length {quality_length}"
            )));
        }
        out.push('@');
        out.push_str(&id);
        if let Some(description) = optional_str(record, "description") {
            out.push(' ');
            out.push_str(&description);
        }
        out.push('\n');
        out.push_str(&sequence);
        out.push('\n');
        out.push_str("+\n");
        out.push_str(&quality);
        out.push('\n');
    }
    Ok(out.into_bytes())
}

fn bed_bytes(records: &[Map<String, Value>], target: CoordinateSystem) -> OutputResult<Vec<u8>> {
    let mut out = String::new();
    for (index, record) in records.iter().enumerate() {
        let chrom = required_str(record, "chrom", index)?;
        let start = required_u64(record, "start", index)?;
        let end = required_u64(record, "end", index)?;
        if end < start {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: BED end {end} precedes start {start}"
            )));
        }
        let mut fields = vec![
            chrom,
            target.output_start(start)?.to_string(),
            end.to_string(),
        ];
        let optional = &BED_COLUMNS[3..];
        let last_present = optional
            .iter()
            .rposition(|column| !record.get(*column).unwrap_or(&Value::Null).is_null());
        if let Some(last_present) = last_present {
            for column in &optional[..=last_present] {
                let value = record.get(*column).unwrap_or(&Value::Null);
                if value.is_null() {
                    return Err(OutputError::InvalidRecord(format!(
                        "record {index}: BED column {:?} is missing but later columns are \
                         present; BED fields must be filled left to right",
                        column
                    )));
                }
                fields.push(bed_cell(value));
            }
        }
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

fn bed_cell(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(values) => values.iter().map(bed_cell).collect::<Vec<_>>().join(","),
        Value::Null => String::new(),
        scalar => scalar.to_string(),
    }
}

fn gff3_bytes(records: &[Map<String, Value>], target: CoordinateSystem) -> OutputResult<Vec<u8>> {
    let mut out = String::from("##gff-version 3\n");
    let mut previous_seqid: Option<String> = None;
    for (index, record) in records.iter().enumerate() {
        let seqid = required_str(record, "seqid", index)?;
        if previous_seqid
            .as_deref()
            .is_some_and(|previous| previous != seqid)
        {
            out.push_str("###\n");
        }
        let start = required_u64(record, "start", index)?;
        let end = required_u64(record, "end", index)?;
        if end < start {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: GFF3 end {end} precedes start {start}"
            )));
        }
        let fields = [
            seqid.clone(),
            required_str_or_dot(record, "source", index)?,
            required_str_or_dot(record, "type", index)?,
            target.output_start(start)?.to_string(),
            end.to_string(),
            optional_number_or_dot(record, "score", index)?,
            optional_str(record, "strand").unwrap_or_else(|| ".".to_owned()),
            optional_phase(record, "phase", index)?,
            gff3_attributes(record, index)?,
        ];
        out.push_str(&fields.join("\t"));
        out.push('\n');
        previous_seqid = Some(seqid);
    }
    Ok(out.into_bytes())
}

/// Percent-encodes the characters the GFF3 specification reserves inside
/// attribute keys and values.
fn gff3_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '%' => out.push_str("%25"),
            ';' => out.push_str("%3B"),
            '=' => out.push_str("%3D"),
            '&' => out.push_str("%26"),
            ',' => out.push_str("%2C"),
            '\t' => out.push_str("%09"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            other => out.push(other),
        }
    }
    out
}

fn gff3_attributes(record: &Map<String, Value>, index: usize) -> OutputResult<String> {
    let Some(Value::Object(attributes)) = record.get("attributes") else {
        return Ok(".".to_owned());
    };
    if attributes.is_empty() {
        return Ok(".".to_owned());
    }
    let mut parts = Vec::with_capacity(attributes.len());
    for (key, value) in attributes {
        match value {
            Value::Null | Value::Bool(false) => continue,
            Value::Bool(true) => parts.push(format!("{}=", gff3_escape(key))),
            Value::Array(values) => {
                let rendered = values
                    .iter()
                    .map(|item| scalar_text(item, index))
                    .collect::<OutputResult<Vec<_>>>()?
                    .iter()
                    .map(|text| gff3_escape(text))
                    .collect::<Vec<_>>()
                    .join(",");
                parts.push(format!("{}={}", gff3_escape(key), rendered));
            }
            other => parts.push(format!(
                "{}={}",
                gff3_escape(key),
                gff3_escape(&scalar_text(other, index)?)
            )),
        }
    }
    if parts.is_empty() {
        return Ok(".".to_owned());
    }
    Ok(parts.join(";"))
}

fn scalar_text(value: &Value, index: usize) -> OutputResult<String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(_) | Value::Bool(_) => Ok(value.to_string()),
        other => Err(OutputError::InvalidRecord(format!(
            "record {index}: nested arrays and objects are not canonical attribute values, \
             got {other:?}"
        ))),
    }
}

fn gtf_bytes(records: &[Map<String, Value>], target: CoordinateSystem) -> OutputResult<Vec<u8>> {
    let mut out = String::new();
    for (index, record) in records.iter().enumerate() {
        let seqid = required_str(record, "seqid", index)?;
        let start = required_u64(record, "start", index)?;
        let end = required_u64(record, "end", index)?;
        if end < start {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: GTF end {end} precedes start {start}"
            )));
        }
        let fields = [
            seqid,
            required_str_or_dot(record, "source", index)?,
            required_str_or_dot(record, "type", index)?,
            target.output_start(start)?.to_string(),
            end.to_string(),
            optional_number_or_dot(record, "score", index)?,
            optional_str(record, "strand").unwrap_or_else(|| ".".to_owned()),
            optional_phase(record, "frame", index)?,
            gtf_attributes(record, index)?,
        ];
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

fn gtf_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn gtf_attributes(record: &Map<String, Value>, index: usize) -> OutputResult<String> {
    let Some(Value::Object(attributes)) = record.get("attributes") else {
        return Ok(".".to_owned());
    };
    if attributes.is_empty() {
        return Ok(".".to_owned());
    }
    let mut parts = Vec::with_capacity(attributes.len());
    for (key, value) in attributes {
        let rendered = match value {
            Value::Null => continue,
            Value::String(text) => format!("\"{}\"", gtf_escape(text)),
            Value::Array(_) => format!("\"{}\"", gtf_escape(&serde_json::to_string(value)?)),
            other => scalar_text(other, index)?,
        };
        parts.push(format!("{key} {rendered};"));
    }
    if parts.is_empty() {
        return Ok(".".to_owned());
    }
    Ok(parts.join(" "))
}

fn vcf_bytes(
    records: &[Map<String, Value>],
    target: CoordinateSystem,
    options: &WriteOptions,
) -> OutputResult<Vec<u8>> {
    let mut info_columns: Vec<(String, &'static str)> = Vec::new();
    let mut filter_ids: Vec<String> = Vec::new();
    let mut format_ids: Vec<String> = Vec::new();
    for record in records {
        if let Some(Value::Object(info)) = record.get("info") {
            for (key, value) in info {
                let kind = info_value_kind(value);
                if let Some(kind) = kind {
                    upsert_ordered(&mut info_columns, key.clone(), kind);
                }
            }
        }
        match record.get("filter") {
            Some(Value::String(text)) if text != "." && text != "PASS" => {
                ensure_ordered(&mut filter_ids, text.clone())
            }
            Some(Value::Array(values)) => {
                for value in values {
                    if let Value::String(text) = value
                        && text != "."
                        && text != "PASS"
                    {
                        ensure_ordered(&mut filter_ids, text.clone());
                    }
                }
            }
            _ => {}
        }
        if let Some(Value::String(format)) = record.get("format") {
            for token in format.split(':') {
                if !token.is_empty() {
                    ensure_ordered(&mut format_ids, token.to_owned());
                }
            }
        }
    }

    let sample_names = match records.first() {
        None => Vec::new(),
        Some(first) => match first.get("samples") {
            Some(Value::Array(samples)) => {
                let count = samples.len();
                match &options.sample_names {
                    Some(names) if names.len() != count => {
                        return Err(OutputError::InvalidRecord(format!(
                            "the spec provides {} sample names but the records carry {count} \
                             sample columns",
                            names.len()
                        )));
                    }
                    Some(names) => names.clone(),
                    None => (1..=count).map(|index| format!("SAMPLE{index}")).collect(),
                }
            }
            Some(_) => {
                return Err(OutputError::InvalidRecord(
                    "VCF `samples` must be an array of sample strings".to_owned(),
                ));
            }
            None => Vec::new(),
        },
    };

    let mut out = String::from("##fileformat=VCFv4.2\n");
    for line in options.header.iter().flatten() {
        out.push_str(line);
        out.push('\n');
    }
    for (id, kind) in &info_columns {
        out.push_str(&format!(
            "##INFO=<ID={id},Number=.,Type={kind},Description=\"Synthesized by Linxira Bio export\">\n"
        ));
    }
    for id in &filter_ids {
        out.push_str(&format!(
            "##FILTER=<ID={id},Description=\"Synthesized by Linxira Bio export\">\n"
        ));
    }
    for id in &format_ids {
        out.push_str(&format!(
            "##FORMAT=<ID={id},Number=.,Type=String,Description=\"Synthesized by Linxira Bio export\">\n"
        ));
    }

    let mut columns = match &options.columns {
        Some(columns) => columns.clone(),
        None => {
            let mut columns = vec![
                "#CHROM".to_owned(),
                "POS".to_owned(),
                "ID".to_owned(),
                "REF".to_owned(),
                "ALT".to_owned(),
                "QUAL".to_owned(),
                "FILTER".to_owned(),
                "INFO".to_owned(),
            ];
            if !sample_names.is_empty() {
                columns.push("FORMAT".to_owned());
                columns.extend(sample_names.iter().cloned());
            }
            columns
        }
    };
    if let Some(first) = columns.first_mut() {
        if first != "#CHROM" {
            return Err(OutputError::InvalidSpec(
                "the VCF column override must start with #CHROM".to_owned(),
            ));
        }
    } else {
        return Err(OutputError::InvalidSpec(
            "the VCF column override must not be empty".to_owned(),
        ));
    }
    out.push_str(&columns.join("\t"));
    out.push('\n');

    let mut expected_samples = sample_names.len();
    for (index, record) in records.iter().enumerate() {
        let chrom = required_str(record, "chrom", index)?;
        let pos = required_u64(record, "pos", index)?;
        let reference = required_str(record, "ref", index)?;
        let samples = match record.get("samples") {
            Some(Value::Array(samples)) => samples
                .iter()
                .map(|sample| scalar_text(sample, index))
                .collect::<OutputResult<Vec<_>>>()?,
            None => Vec::new(),
            Some(_) => {
                return Err(OutputError::InvalidRecord(format!(
                    "record {index}: VCF `samples` must be an array of sample strings"
                )));
            }
        };
        let format = optional_str(record, "format");
        if format.is_some() != !samples.is_empty() {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: VCF FORMAT and sample columns must be provided together"
            )));
        }
        if index == 0 {
            expected_samples = samples.len();
        } else if samples.len() != expected_samples {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: VCF records must share one sample column count \
                 (expected {expected_samples}, got {})",
                samples.len()
            )));
        }

        let mut fields = vec![
            chrom,
            target.output_start(pos)?.to_string(),
            optional_str(record, "id").unwrap_or_else(|| ".".to_owned()),
            reference,
            vcf_alt(record, index)?,
            optional_number_or_dot(record, "qual", index)?,
            vcf_filter(record)?,
            vcf_info(record, index)?,
        ];
        if let Some(format) = format {
            fields.push(format);
            fields.extend(samples);
        }
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

fn upsert_ordered<T>(ordered: &mut Vec<(String, T)>, key: String, value: T) {
    if let Some(entry) = ordered.iter_mut().find(|(existing, _)| *existing == key) {
        entry.1 = value;
    } else {
        ordered.push((key, value));
    }
}

fn ensure_ordered(ordered: &mut Vec<String>, key: String) {
    if !ordered.contains(&key) {
        ordered.push(key);
    }
}

fn info_value_kind(value: &Value) -> Option<&'static str> {
    match value {
        Value::Bool(_) => Some("Flag"),
        Value::Number(number) => {
            if number.as_i64().is_some() || number.as_u64().is_some() {
                Some("Integer")
            } else {
                Some("Float")
            }
        }
        Value::String(_) | Value::Array(_) => Some("String"),
        Value::Null => None,
        Value::Object(_) => None,
    }
}

fn vcf_alt(record: &Map<String, Value>, index: usize) -> OutputResult<String> {
    match record.get("alt") {
        None | Some(Value::Null) => Ok(".".to_owned()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| scalar_text(value, index))
            .collect::<OutputResult<Vec<_>>>()
            .map(|items| items.join(",")),
        Some(_) => Err(OutputError::InvalidRecord(format!(
            "record {index}: VCF `alt` must be a string or an array of allele strings"
        ))),
    }
}

fn vcf_filter(record: &Map<String, Value>) -> OutputResult<String> {
    match record.get("filter") {
        None | Some(Value::Null) => Ok(".".to_owned()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(Value::Array(values)) => {
            let mut filters = Vec::with_capacity(values.len());
            for value in values {
                match value {
                    Value::String(text) => filters.push(text.clone()),
                    other => {
                        return Err(OutputError::InvalidRecord(format!(
                            "VCF filter entries must be strings, got {other:?}"
                        )));
                    }
                }
            }
            Ok(filters.join(";"))
        }
        Some(_) => Err(OutputError::InvalidRecord(
            "VCF `filter` must be a string or an array of filter identifiers".to_owned(),
        )),
    }
}

fn vcf_info(record: &Map<String, Value>, index: usize) -> OutputResult<String> {
    match record.get("info") {
        None | Some(Value::Null) => Ok(".".to_owned()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(Value::Object(info)) => {
            let mut parts = Vec::with_capacity(info.len());
            for (key, value) in info {
                match value {
                    Value::Null | Value::Bool(false) => continue,
                    Value::Bool(true) => parts.push(key.clone()),
                    Value::Array(values) => {
                        let rendered = values
                            .iter()
                            .map(|item| scalar_text(item, index))
                            .collect::<OutputResult<Vec<_>>>()?
                            .join(",");
                        parts.push(format!("{key}={rendered}"));
                    }
                    other => parts.push(format!("{key}={}", scalar_text(other, index)?)),
                }
            }
            if parts.is_empty() {
                Ok(".".to_owned())
            } else {
                Ok(parts.join(";"))
            }
        }
        Some(_) => Err(OutputError::InvalidRecord(format!(
            "record {index}: VCF `info` must be an object or a preformatted string"
        ))),
    }
}

fn sam_bytes(records: &[Map<String, Value>], options: &WriteOptions) -> OutputResult<Vec<u8>> {
    let mut out = String::new();
    let header_lines: Vec<String> = options.header.iter().flatten().cloned().collect();
    let declares_hd = header_lines
        .first()
        .is_some_and(|line| line.starts_with("@HD"));
    if !declares_hd {
        out.push_str("@HD\tVN:1.6\tSO:unsorted\n");
    }
    for line in &header_lines {
        out.push_str(line);
        out.push('\n');
    }
    for (index, record) in records.iter().enumerate() {
        let qname = required_str(record, "qname", index)?;
        let flag = required_u64(record, "flag", index)?;
        if flag > u16::MAX as u64 {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: SAM flag {flag} exceeds the 16-bit flag field"
            )));
        }
        let pos = required_u64(record, "pos", index)?;
        // POS 0 marks an unmapped read and must not shift into 1-based space.
        let rendered_pos = if pos == 0 {
            0
        } else {
            CoordinateSystem::Gff.output_start(pos)?
        };
        let sequence = required_str(record, "seq", index)?;
        let quality = required_str(record, "qual", index)?;
        if quality != "*" && quality.chars().count() != sequence.chars().count() {
            return Err(OutputError::InvalidRecord(format!(
                "record {index}: SAM sequence and quality lengths differ"
            )));
        }
        let mut fields = vec![
            qname,
            flag.to_string(),
            optional_str(record, "rname").unwrap_or_else(|| "*".to_owned()),
            rendered_pos.to_string(),
            optional_u64(record, "mapq").unwrap_or(0).to_string(),
            optional_str(record, "cigar").unwrap_or_else(|| "*".to_owned()),
            optional_str(record, "rnext").unwrap_or_else(|| "*".to_owned()),
            optional_u64(record, "pnext").unwrap_or(0).to_string(),
            optional_i64(record, "tlen").unwrap_or(0).to_string(),
            sequence,
            quality,
        ];
        match record.get("tags") {
            Some(Value::Array(tags)) => {
                for tag in tags {
                    fields.push(scalar_text(tag, index)?);
                }
            }
            Some(Value::String(tag)) => fields.push(tag.clone()),
            Some(_) => {
                return Err(OutputError::InvalidRecord(format!(
                    "record {index}: SAM `tags` must be an array of tag strings"
                )));
            }
            None => {}
        }
        out.push_str(&fields.join("\t"));
        out.push('\n');
    }
    Ok(out.into_bytes())
}

fn required_str(record: &Map<String, Value>, field: &str, index: usize) -> OutputResult<String> {
    match record.get(field) {
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => Err(OutputError::InvalidRecord(format!(
            "record {index}: field {field:?} must be a string, got {other:?}"
        ))),
        None => Err(OutputError::InvalidRecord(format!(
            "record {index} is missing the required {field:?} field"
        ))),
    }
}

fn required_str_or_dot(
    record: &Map<String, Value>,
    field: &str,
    index: usize,
) -> OutputResult<String> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(".".to_owned()),
        Some(Value::String(text)) => Ok(text.clone()),
        Some(other) => Err(OutputError::InvalidRecord(format!(
            "record {index}: field {field:?} must be a string, got {other:?}"
        ))),
    }
}

fn optional_str(record: &Map<String, Value>, field: &str) -> Option<String> {
    record
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn required_u64(record: &Map<String, Value>, field: &str, index: usize) -> OutputResult<u64> {
    match record.get(field) {
        Some(Value::Number(number)) => number.as_u64().ok_or_else(|| {
            OutputError::InvalidRecord(format!(
                "record {index}: field {field:?} must be a non-negative integer"
            ))
        }),
        Some(other) => Err(OutputError::InvalidRecord(format!(
            "record {index}: field {field:?} must be a number, got {other:?}"
        ))),
        None => Err(OutputError::InvalidRecord(format!(
            "record {index} is missing the required {field:?} field"
        ))),
    }
}

fn optional_u64(record: &Map<String, Value>, field: &str) -> Option<u64> {
    record.get(field).and_then(Value::as_u64)
}

fn optional_i64(record: &Map<String, Value>, field: &str) -> Option<i64> {
    record.get(field).and_then(Value::as_i64)
}

/// Renders a numeric field, or `.` when the field is absent, null, or the
/// literal `"."` (the missing-value marker of GFF3/GTF/VCF).
fn optional_number_or_dot(
    record: &Map<String, Value>,
    field: &str,
    index: usize,
) -> OutputResult<String> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(".".to_owned()),
        Some(Value::String(text)) if text == "." => Ok(".".to_owned()),
        Some(Value::Number(number)) => Ok(number.to_string()),
        Some(other) => Err(OutputError::InvalidRecord(format!(
            "record {index}: field {field:?} must be a number or \".\", got {other:?}"
        ))),
    }
}

/// Renders the GFF3 `phase` / GTF `frame` column: 0-2, or `.` when absent.
fn optional_phase(record: &Map<String, Value>, field: &str, index: usize) -> OutputResult<String> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(".".to_owned()),
        Some(Value::String(text)) if text == "." => Ok(".".to_owned()),
        Some(Value::Number(number)) => match number.as_u64() {
            Some(phase @ 0..=2) => Ok(phase.to_string()),
            other => Err(OutputError::InvalidRecord(format!(
                "record {index}: field {field:?} must be 0, 1, or 2, got {other:?}"
            ))),
        },
        Some(other) => Err(OutputError::InvalidRecord(format!(
            "record {index}: field {field:?} must be 0, 1, 2, or \".\", got {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{BioDataWriter, DEFAULT_FASTA_LINE_WIDTH};
    use crate::{CoordinateSystem, WriteOptions};
    use serde_json::json;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "linxira-bio-output-writers-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temporary root");
        root
    }

    #[test]
    fn writes_fasta_with_standard_wrap_and_descriptions() {
        let long_sequence = "A".repeat(DEFAULT_FASTA_LINE_WIDTH + 5);
        let value = json!([
            {"id": "seq1", "description": "first record", "sequence": "ACGT"},
            {"id": "seq2", "sequence": long_sequence}
        ]);
        let root = temp_root("fasta");
        let output = root.join("contigs.fa");
        let receipt = BioDataWriter::write_to_path(&value, &output, &WriteOptions::default())
            .expect("FASTA write");
        assert_eq!(receipt.format, linxira_bio_protocol::BioDataFormat::Fasta);

        let text = std::fs::read_to_string(&output).expect("read FASTA");
        let expected = format!(
            ">seq1 first record\nACGT\n>seq2\n{}\n{}\n",
            "A".repeat(DEFAULT_FASTA_LINE_WIDTH),
            "A".repeat(5)
        );
        assert_eq!(text, expected);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn fasta_write_rejects_zero_width_without_creating_output() {
        let value = json!([{"id": "s", "sequence": "AC"}]);
        let root = temp_root("fasta-width");
        let output = root.join("contigs.fa");
        let options = WriteOptions {
            fasta_line_width: Some(0),
            ..WriteOptions::default()
        };
        let error = BioDataWriter::write_to_path(&value, &output, &options)
            .expect_err("zero width must fail");
        assert!(error.to_string().contains("line width"));
        assert!(!output.exists());
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_fastq_and_validates_quality_length() {
        let value = json!([
            {"id": "r1", "sequence": "ACGT", "quality": "IIII"},
            {"id": "r2", "description": "paired mate", "sequence": "AA", "quality": "##"}
        ]);
        let root = temp_root("fastq");
        let output = root.join("reads.fq");
        BioDataWriter::write_to_path(&value, &output, &WriteOptions::default())
            .expect("FASTQ write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read FASTQ"),
            "@r1\nACGT\n+\nIIII\n@r2 paired mate\nAA\n+\n##\n"
        );

        let mismatch = json!([{"id": "r3", "sequence": "ACGT", "quality": "II"}]);
        let error = BioDataWriter::write_to_path(&mismatch, &output, &WriteOptions::default())
            .expect_err("quality length mismatch");
        assert!(error.to_string().contains("does not match quality length"));
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_bed_with_boundary_coordinates_and_trailing_field_trimming() {
        let value = json!([
            {"chrom": "chr1", "start": 1, "end": 5},
            {"chrom": "chr1", "start": 10, "end": 20, "name": "site", "score": 500, "strand": "+"}
        ]);
        let root = temp_root("bed");
        let output = root.join("intervals.bed");
        BioDataWriter::write_to_path(&value, &output, &WriteOptions::default()).expect("BED write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read BED"),
            "chr1\t0\t5\nchr1\t9\t20\tsite\t500\t+\n"
        );

        // A GFF-convention override keeps the 1-based start as-is.
        let options = WriteOptions {
            coords: Some(CoordinateSystem::Gff),
            ..WriteOptions::default()
        };
        BioDataWriter::write_to_path(&value, &output, &options).expect("BED write (1-based)");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read BED"),
            "chr1\t1\t5\nchr1\t10\t20\tsite\t500\t+\n"
        );

        let zero_based = json!([{"chrom": "chr1", "start": 0, "end": 5}]);
        let error = BioDataWriter::write_to_path(&zero_based, &output, &WriteOptions::default())
            .expect_err("1-based JSON cannot express bed start -1");
        assert!(error.to_string().contains("1-based closed"));
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn bed_rejects_gapped_optional_columns() {
        let value = json!([{"chrom": "chr1", "start": 1, "end": 5, "strand": "+"}]);
        let root = temp_root("bed-gap");
        let output = root.join("intervals.bed");
        let error = BioDataWriter::write_to_path(&value, &output, &WriteOptions::default())
            .expect_err("strand without name must fail");
        assert!(error.to_string().contains("left to right"));
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_gff3_with_header_encoding_and_seqid_separators() {
        let value = json!([
            {"seqid": "chr1", "source": "engine", "type": "gene", "start": 1, "end": 100,
             "score": 12.5, "strand": "+", "phase": 0,
             "attributes": {"ID": "gene1", "Name": "brca;1"}},
            {"seqid": "chr2", "source": "engine", "type": "exon", "start": 50, "end": 60}
        ]);
        let root = temp_root("gff3");
        let output = root.join("genes.gff3");
        BioDataWriter::write_to_path(&value, &output, &WriteOptions::default())
            .expect("GFF3 write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read GFF3"),
            "##gff-version 3\n\
             chr1\tengine\tgene\t1\t100\t12.5\t+\t0\tID=gene1;Name=brca%3B1\n\
             ###\n\
             chr2\tengine\texon\t50\t60\t.\t.\t.\t.\n"
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_gtf_with_quoted_and_bare_attributes() {
        let value = json!([
            {"seqid": "chr1", "source": "engine", "type": "exon", "start": 1, "end": 100,
             "strand": "+", "frame": 0,
             "attributes": {"gene_id": "G1", "exon_number": 2, "orthologous": true}}
        ]);
        let root = temp_root("gtf");
        let output = root.join("transcripts.gtf");
        BioDataWriter::write_to_path(&value, &output, &WriteOptions::default()).expect("GTF write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read GTF"),
            "chr1\tengine\texon\t1\t100\t.\t+\t0\t\
             exon_number 2; gene_id \"G1\"; orthologous true;\n"
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_vcf_with_synthesized_meta_and_sample_columns() {
        let value = json!([
            {"chrom": "chr1", "pos": 1, "id": "rs1", "ref": "A", "alt": "G",
             "qual": 50, "filter": "PASS", "info": {"dp": 31, "somatic": true},
             "format": "GT:DP", "samples": ["0/1:12", "1/1:8"]},
            {"chrom": "chr1", "pos": 20, "ref": "C", "alt": ["T", "G"], "qual": 12.5,
             "filter": ["q10", "s5"], "info": {"af": 0.5},
             "format": "GT:DP", "samples": ["0/1:14", "1/1:9"]}
        ]);
        let root = temp_root("vcf");
        let output = root.join("variants.vcf");
        BioDataWriter::write_to_path(&value, &output, &WriteOptions::default()).expect("VCF write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read VCF"),
            "##fileformat=VCFv4.2\n\
             ##INFO=<ID=dp,Number=.,Type=Integer,Description=\"Synthesized by Linxira Bio export\">\n\
             ##INFO=<ID=somatic,Number=.,Type=Flag,Description=\"Synthesized by Linxira Bio export\">\n\
             ##INFO=<ID=af,Number=.,Type=Float,Description=\"Synthesized by Linxira Bio export\">\n\
             ##FILTER=<ID=q10,Description=\"Synthesized by Linxira Bio export\">\n\
             ##FILTER=<ID=s5,Description=\"Synthesized by Linxira Bio export\">\n\
             ##FORMAT=<ID=GT,Number=.,Type=String,Description=\"Synthesized by Linxira Bio export\">\n\
             ##FORMAT=<ID=DP,Number=.,Type=String,Description=\"Synthesized by Linxira Bio export\">\n\
             #CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tSAMPLE1\tSAMPLE2\n\
             chr1\t1\trs1\tA\tG\t50\tPASS\tdp=31;somatic\tGT:DP\t0/1:12\t1/1:8\n\
             chr1\t20\t.\tC\tT,G\t12.5\tq10;s5\taf=0.5\tGT:DP\t0/1:14\t1/1:9\n"
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn vcf_rejects_format_without_samples() {
        let value = json!([{"chrom": "chr1", "pos": 1, "ref": "A", "format": "GT"}]);
        let root = temp_root("vcf-xor");
        let output = root.join("variants.vcf");
        let error = BioDataWriter::write_to_path(&value, &output, &WriteOptions::default())
            .expect_err("FORMAT without samples must fail");
        assert!(error.to_string().contains("together"));
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_sam_with_default_header_tags_and_length_check() {
        let value = json!([
            {"qname": "read1", "flag": 0, "rname": "chr1", "pos": 1, "mapq": 60,
             "cigar": "4M", "rnext": "*", "pnext": 0, "tlen": 0,
             "seq": "ACGT", "qual": "IIII", "tags": ["NM:i:0", "MD:Z:4"]},
            {"qname": "read2", "flag": 4, "pos": 0, "seq": "GG", "qual": "*"}
        ]);
        let root = temp_root("sam");
        let output = root.join("alignments.sam");
        BioDataWriter::write_to_path(&value, &output, &WriteOptions::default()).expect("SAM write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read SAM"),
            "@HD\tVN:1.6\tSO:unsorted\n\
             read1\t0\tchr1\t1\t60\t4M\t*\t0\t0\tACGT\tIIII\tNM:i:0\tMD:Z:4\n\
             read2\t4\t*\t0\t0\t*\t*\t0\t0\tGG\t*\n"
        );

        let mismatch = json!([{"qname": "r", "flag": 0, "pos": 1, "seq": "ACGT", "qual": "II"}]);
        let error = BioDataWriter::write_to_path(&mismatch, &output, &WriteOptions::default())
            .expect_err("sequence/quality mismatch must fail");
        assert!(error.to_string().contains("lengths differ"));
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn failed_writes_never_replace_existing_artifacts() {
        let root = temp_root("atomic");
        let output = root.join("intervals.bed");
        std::fs::write(&output, "stale but complete\n").expect("write stale artifact");

        let broken = json!([{"chrom": "chr1", "start": "x", "end": 5}]);
        let error = BioDataWriter::write_to_path(&broken, &output, &WriteOptions::default())
            .expect_err("non-numeric start must fail");
        assert!(error.to_string().contains("must be a number"));
        assert_eq!(
            std::fs::read_to_string(&output).expect("existing artifact survives"),
            "stale but complete\n"
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn writes_tabular_formats_through_the_export_crate() {
        let value = json!([
            {"sample": "A", "count": 4},
            {"sample": "B", "count": 9}
        ]);
        let root = temp_root("tables");
        let columns = Some(vec!["sample".to_owned(), "count".to_owned()]);
        let options = WriteOptions {
            columns,
            ..WriteOptions::default()
        };
        let output = root.join("counts.csv");
        BioDataWriter::write_to_path(&value, &output, &options).expect("csv write");
        assert_eq!(
            std::fs::read_to_string(&output).expect("read csv"),
            "sample,count\nA,4\nB,9\n"
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn unknown_and_plot_formats_are_rejected() {
        let root = temp_root("unsupported");
        let value = json!([]);
        assert!(
            BioDataWriter::write_to_path(&value, &root.join("plot.svg"), &WriteOptions::default())
                .is_err()
        );
        assert!(
            BioDataWriter::write_to_path(
                &value,
                &root.join("data.parquet"),
                &WriteOptions::default()
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }
}
