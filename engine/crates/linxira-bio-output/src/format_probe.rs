//! Input format and compression probing with automatic decompression
//! (M0-T9..T11).
//!
//! `probe_format` identifies the innermost content format and its
//! compression container from magic bytes first and the file-name extension
//! second, so disguised inputs (a gzip stream named `.fa`, a 7z archive
//! named `.gz`) are still reported correctly. `ensure_decompressed` streams
//! gzip containers through flate2 and hands archives to native multi-thread
//! `7z`/`7zz` when available, in line with the repository's native-first
//! rule.

use crate::{OutputError, OutputResult, timestamp_suffix};
use linxira_bio_protocol::BioDataFormat;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Compression container detected while probing. Richer than the protocol
/// enum: probing must distinguish 7z archives before choosing an unpacking
/// strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeCompression {
    None,
    Gzip,
    Bgzip,
    Bzip2,
    Xz,
    Zstd,
    Zip,
    SevenZip,
    Unknown,
}

impl ProbeCompression {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Gzip => "gzip",
            Self::Bgzip => "bgzip",
            Self::Bzip2 => "bzip2",
            Self::Xz => "xz",
            Self::Zstd => "zstd",
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Unknown => "unknown",
        }
    }
}

/// How the probe identified the artifact: from content (magic or sniffed
/// text), from the file-name extension, or both in agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeConfidence {
    Magic,
    Extension,
    Both,
}

impl ProbeConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Magic => "magic",
            Self::Extension => "extension",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    /// The innermost content format; `Unknown` when only the container
    /// could be identified.
    pub format: BioDataFormat,
    pub compression: ProbeCompression,
    pub confidence: ProbeConfidence,
    /// True when the input looks like an NCBI SRA archive; unpacking needs
    /// the `sra-tools` suite (see [`unpack_sra`]).
    pub sra: bool,
}

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
const BZIP2_MAGIC: [u8; 3] = *b"BZh";
const XZ_MAGIC: [u8; 6] = [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00];
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];
const ZIP_MAGIC: [u8; 4] = *b"PK\x03\x04";
const SEVENZIP_MAGIC: [u8; 6] = [0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c];
const BAM_MAGIC: [u8; 4] = *b"BAM\x01";
const SRA_MAGIC: [u8; 4] = *b"NCBI";
const HEAD_BYTES: usize = 4096;

/// Probes a path for its content format and compression container.
pub fn probe_format(path: &Path) -> OutputResult<ProbeResult> {
    let head = read_head(path)?;
    let extension_format = extension_format(path);

    if head.starts_with(&GZIP_MAGIC) {
        let compression = if is_bgzf(&head) {
            ProbeCompression::Bgzip
        } else {
            ProbeCompression::Gzip
        };
        let inner = sniff_gzip_head(path);
        return Ok(compressed_result(compression, inner, extension_format));
    }
    for (magic, compression) in [
        (&BZIP2_MAGIC[..], ProbeCompression::Bzip2),
        (&XZ_MAGIC[..], ProbeCompression::Xz),
        (&ZSTD_MAGIC[..], ProbeCompression::Zstd),
        (&ZIP_MAGIC[..], ProbeCompression::Zip),
        (&SEVENZIP_MAGIC[..], ProbeCompression::SevenZip),
    ] {
        if head.starts_with(magic) {
            // Non-gzip containers cannot be sniffed without unpacking; the
            // inner format falls back to the file name.
            return Ok(compressed_result(compression, None, extension_format));
        }
    }
    if head.starts_with(&BAM_MAGIC) {
        return Ok(agreed_result(BioDataFormat::Bam, extension_format, false));
    }
    let sra_magic = head.starts_with(&SRA_MAGIC);
    if sra_magic || extension_is(path, &["sra"]) {
        return Ok(ProbeResult {
            format: BioDataFormat::Unknown,
            compression: ProbeCompression::Unknown,
            confidence: if sra_magic {
                ProbeConfidence::Magic
            } else {
                ProbeConfidence::Extension
            },
            sra: true,
        });
    }

    match (sniff_content(&head), extension_format) {
        (Some(format), _) => Ok(agreed_result(format, extension_format, false)),
        (None, Some(format)) => Ok(ProbeResult {
            format,
            compression: ProbeCompression::None,
            confidence: ProbeConfidence::Extension,
            sra: false,
        }),
        (None, None) => Ok(ProbeResult {
            format: BioDataFormat::Unknown,
            compression: ProbeCompression::None,
            confidence: ProbeConfidence::Extension,
            sra: false,
        }),
    }
}

/// Assembles the probe verdict for a compressed container. When the inner
/// content was sniffed it wins over the file name; when both agree the
/// confidence is `Both`.
fn compressed_result(
    compression: ProbeCompression,
    inner: Option<BioDataFormat>,
    extension_format: Option<BioDataFormat>,
) -> ProbeResult {
    match (inner, extension_format) {
        (Some(format), extension) => ProbeResult {
            format,
            compression,
            confidence: if extension == Some(format) {
                ProbeConfidence::Both
            } else {
                ProbeConfidence::Magic
            },
            sra: false,
        },
        (None, Some(format)) => ProbeResult {
            format,
            compression,
            confidence: ProbeConfidence::Extension,
            sra: false,
        },
        (None, None) => ProbeResult {
            format: BioDataFormat::Unknown,
            compression,
            confidence: ProbeConfidence::Magic,
            sra: false,
        },
    }
}

fn agreed_result(
    format: BioDataFormat,
    extension_format: Option<BioDataFormat>,
    sra: bool,
) -> ProbeResult {
    let confidence = if extension_format == Some(format) {
        ProbeConfidence::Both
    } else {
        ProbeConfidence::Magic
    };
    ProbeResult {
        format,
        compression: ProbeCompression::None,
        confidence,
        sra,
    }
}

fn read_head(path: &Path) -> OutputResult<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut head = Vec::with_capacity(HEAD_BYTES);
    file.take(HEAD_BYTES as u64)
        .read_to_end(&mut head)
        .map_err(OutputError::Io)?;
    Ok(head)
}

/// Decompresses just enough of a gzip stream to sniff the inner content.
/// A corrupt tail must not defeat the probe, so read errors after the first
/// bytes are tolerated.
fn sniff_gzip_head(path: &Path) -> Option<BioDataFormat> {
    let file = std::fs::File::open(path).ok()?;
    let mut decoder = flate2::read::MultiGzDecoder::new(file);
    let mut head = Vec::with_capacity(HEAD_BYTES);
    let mut buffer = [0u8; 1024];
    loop {
        match decoder.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                head.extend_from_slice(&buffer[..read]);
                if head.len() >= HEAD_BYTES {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    sniff_content(&head)
}

/// BGZF is a gzip member carrying the `BC` extra subfield right after the
/// fixed header.
fn is_bgzf(head: &[u8]) -> bool {
    head.len() >= 18 && (head[3] & 0x04) != 0 && head[12] == b'B' && head[13] == b'C'
}

/// Extends the writer extension map with the read-only formats probing must
/// recognize (BAM/CRAM/BCF/PDB/mmCIF) and walks compression suffixes, so
/// `reads.fastq.gz` resolves to FASTQ and `archive.7z` to nothing.
fn extension_format(path: &Path) -> Option<BioDataFormat> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut current = name.as_str();
    while let Some(dot) = current.rfind('.') {
        let extension = &current[dot + 1..];
        current = &current[..dot];
        if matches!(
            extension,
            "gz" | "bgz" | "bz2" | "xz" | "zst" | "zip" | "7z"
        ) {
            continue;
        }
        return lookup_format_extension(extension);
    }
    None
}

fn lookup_format_extension(extension: &str) -> Option<BioDataFormat> {
    match extension {
        "fa" | "fasta" | "fna" | "faa" => Some(BioDataFormat::Fasta),
        "fq" | "fastq" => Some(BioDataFormat::Fastq),
        "bed" => Some(BioDataFormat::Bed),
        "gff" | "gff3" => Some(BioDataFormat::Gff3),
        "gtf" => Some(BioDataFormat::Gtf),
        "vcf" => Some(BioDataFormat::Vcf),
        "bcf" => Some(BioDataFormat::Bcf),
        "sam" => Some(BioDataFormat::Sam),
        "bam" => Some(BioDataFormat::Bam),
        "cram" => Some(BioDataFormat::Cram),
        "pdb" => Some(BioDataFormat::Pdb),
        "cif" | "mmcif" => Some(BioDataFormat::Mmcif),
        "nwk" | "newick" | "tree" => Some(BioDataFormat::Newick),
        "csv" => Some(BioDataFormat::Csv),
        "tsv" => Some(BioDataFormat::Tsv),
        "json" => Some(BioDataFormat::Json),
        "jsonl" => Some(BioDataFormat::Jsonl),
        "xlsx" => Some(BioDataFormat::Xlsx),
        _ => None,
    }
}

fn extension_is(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| extensions.contains(&value.to_ascii_lowercase().as_str()))
}

/// Content sniffing over the first bytes of an (already decompressed) file.
fn sniff_content(head: &[u8]) -> Option<BioDataFormat> {
    let text = std::str::from_utf8(head).ok()?;
    let first_line = text.lines().find(|line| !line.trim().is_empty())?;
    if first_line.starts_with('>') {
        return Some(BioDataFormat::Fasta);
    }
    if first_line.starts_with("@HD\t")
        || first_line.starts_with("@SQ\t")
        || first_line.starts_with("@RG\t")
    {
        return Some(BioDataFormat::Sam);
    }
    if first_line.starts_with("##fileformat=VCF") || first_line.starts_with("#CHROM\t") {
        return Some(BioDataFormat::Vcf);
    }
    if first_line.starts_with("##gff-version") {
        return Some(BioDataFormat::Gff3);
    }
    if first_line.starts_with('@') {
        return Some(BioDataFormat::Fastq);
    }
    let fields: Vec<&str> = first_line.split('\t').collect();
    let numeric = |field: &str| field.trim().parse::<u64>().is_ok();
    if fields.len() == 9 && numeric(fields[3]) && numeric(fields[4]) {
        if fields[8].contains('"') {
            return Some(BioDataFormat::Gtf);
        }
        return Some(BioDataFormat::Gff3);
    }
    if fields.len() >= 3 && numeric(fields[1]) && numeric(fields[2]) {
        return Some(BioDataFormat::Bed);
    }
    None
}

/// Returns the input path unchanged when it is not compressed; otherwise
/// decompresses (or unpacks) it into `workspace_tmp` and returns the
/// produced path. Gzip streams through flate2; SRA archives through
/// `sra-tools`; other archives through native `7z`/`7zz` when installed,
/// with an actionable error otherwise.
pub fn ensure_decompressed(path: &Path, workspace_tmp: &Path) -> OutputResult<PathBuf> {
    let result = probe_format(path)?;
    if result.sra {
        let outputs = unpack_sra(path, workspace_tmp)?;
        return Ok(match outputs.as_slice() {
            [single] => single.clone(),
            _ => workspace_tmp.join(format!("{}_sra", file_stem_or_default(path))),
        });
    }
    match result.compression {
        ProbeCompression::None => Ok(path.to_path_buf()),
        ProbeCompression::Gzip | ProbeCompression::Bgzip => gunzip_to(path, workspace_tmp),
        ProbeCompression::Bzip2
        | ProbeCompression::Xz
        | ProbeCompression::Zstd
        | ProbeCompression::Zip
        | ProbeCompression::SevenZip => extract_with_native_7z(path, workspace_tmp),
        ProbeCompression::Unknown => Err(OutputError::UnsupportedFormat(format!(
            "cannot decompress {:?}: the container could not be identified; \
             inspect it with `import probe`",
            path.display()
        ))),
    }
}

fn gunzip_to(path: &Path, workspace_tmp: &Path) -> OutputResult<PathBuf> {
    std::fs::create_dir_all(workspace_tmp)?;
    let stem = file_stem_or_default(path);
    let target = unique_target(workspace_tmp, &stem, "")?;
    let mut decoder = flate2::read::MultiGzDecoder::new(std::fs::File::open(path)?);
    let mut output = std::fs::File::create(&target)?;
    std::io::copy(&mut decoder, &mut output)?;
    Ok(target)
}

fn file_stem_or_default(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("decompressed")
        .to_owned()
}

fn unique_target(workspace_tmp: &Path, stem: &str, extension: &str) -> OutputResult<PathBuf> {
    let desired = workspace_tmp.join(format!("{stem}{extension}"));
    if !desired.exists() {
        return Ok(desired);
    }
    timestamp_suffix(&desired)
}

/// Locates native `7z`/`7zz` on PATH, plus the Windows default install
/// directory. Returns `None` when 7-Zip is not installed.
pub fn find_native_7z() -> Option<PathBuf> {
    let executables: &[&str] = if cfg!(windows) {
        &["7z.exe", "7za.exe", "7zz.exe"]
    } else {
        &["7zz", "7za", "7z"]
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            for executable in executables {
                let candidate = directory.join(executable);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    if cfg!(windows) {
        for candidate in [
            r"C:\Program Files\7-Zip\7z.exe",
            r"C:\Program Files (x86)\7-Zip\7z.exe",
        ] {
            let candidate = PathBuf::from(candidate);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn extract_with_native_7z(path: &Path, workspace_tmp: &Path) -> OutputResult<PathBuf> {
    let tool = find_native_7z().ok_or_else(|| {
        OutputError::UnsupportedFormat(
            "7-Zip/p7zip is required to unpack this archive; install p7zip (Debian/Arch) \
             or 7-Zip (Windows) and retry"
                .to_owned(),
        )
    })?;
    std::fs::create_dir_all(workspace_tmp)?;
    let stem = file_stem_or_default(path);
    let desired = workspace_tmp.join(format!("{stem}_unpacked"));
    let output_directory = if desired.exists() {
        timestamp_suffix(&desired)?
    } else {
        desired
    };
    std::fs::create_dir_all(&output_directory)?;
    let status = Command::new(&tool)
        .arg("x")
        .arg("-y")
        .arg(format!("-o{}", output_directory.display()))
        .arg(path)
        .status()
        .map_err(|error| {
            OutputError::Io(std::io::Error::other(format!(
                "failed to launch {}: {error}",
                tool.display()
            )))
        })?;
    if !status.success() {
        return Err(OutputError::UnsupportedFormat(format!(
            "7z could not unpack {:?} (exit status {status})",
            path.display()
        )));
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&output_directory)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    Ok(match entries.as_slice() {
        [single] if single.is_file() => single.clone(),
        _ => output_directory,
    })
}

/// The sra-tools executables relevant to local SRR processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SraTools {
    pub fasterq_dump: bool,
    pub fastq_dump: bool,
    pub prefetch: bool,
    pub vdb_config: bool,
}

impl SraTools {
    /// True when at least one dump tool can produce FASTQ output.
    pub fn can_unpack(self) -> bool {
        self.fasterq_dump || self.fastq_dump
    }
}

/// Probes the PATH for `fasterq-dump`, `fastq-dump`, `prefetch`, and
/// `vdb-config`.
pub fn probe_sra_tools() -> SraTools {
    SraTools {
        fasterq_dump: find_on_path("fasterq-dump").is_some(),
        fastq_dump: find_on_path("fastq-dump").is_some(),
        prefetch: find_on_path("prefetch").is_some(),
        vdb_config: find_on_path("vdb-config").is_some(),
    }
}

fn find_on_path(executable: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let suffix = if cfg!(windows) {
        format!("{executable}.exe")
    } else {
        executable.to_owned()
    };
    std::env::split_paths(&paths)
        .map(|directory| directory.join(&suffix))
        .find(|candidate| candidate.is_file())
}

/// Unpacks an SRA archive with `fasterq-dump --split-3` (falling back to
/// `fastq-dump --split-3`), producing one FASTQ per mate in `workspace_tmp`.
pub fn unpack_sra(path: &Path, workspace_tmp: &Path) -> OutputResult<Vec<PathBuf>> {
    let tools = probe_sra_tools();
    let (tool, extra_arguments): (PathBuf, &[&str]) = if tools.fasterq_dump {
        (
            find_on_path("fasterq-dump").expect("probe reported fasterq-dump"),
            &["--split-3"],
        )
    } else if tools.fastq_dump {
        (
            find_on_path("fastq-dump").expect("probe reported fastq-dump"),
            &["--split-3", "--gzip"],
        )
    } else {
        return Err(OutputError::UnsupportedFormat(
            "sra-tools is required to unpack SRA archives; install sra-tools \
             (fasterq-dump) and retry"
                .to_owned(),
        ));
    };
    std::fs::create_dir_all(workspace_tmp)?;
    let stem = file_stem_or_default(path);
    let desired = workspace_tmp.join(format!("{stem}_sra"));
    let output_directory = if desired.exists() {
        timestamp_suffix(&desired)?
    } else {
        desired
    };
    std::fs::create_dir_all(&output_directory)?;

    let status = Command::new(&tool)
        .args(extra_arguments)
        .arg("-O")
        .arg(&output_directory)
        .arg(path)
        .status()
        .map_err(|error| {
            OutputError::Io(std::io::Error::other(format!(
                "failed to launch {}: {error}",
                tool.display()
            )))
        })?;
    if !status.success() {
        return Err(OutputError::UnsupportedFormat(format!(
            "{} could not unpack {:?} (exit status {status})",
            tool.display(),
            path.display()
        )));
    }
    let mut outputs: Vec<PathBuf> = std::fs::read_dir(&output_directory)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|candidate| candidate.is_file())
        .collect();
    outputs.sort();
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::{
        ProbeCompression, ProbeConfidence, SEVENZIP_MAGIC, ZIP_MAGIC, ensure_decompressed,
        extension_format, find_native_7z, is_bgzf, probe_format, probe_sra_tools, sniff_content,
    };
    use linxira_bio_protocol::BioDataFormat;
    use std::io::Write;
    use std::path::Path;

    fn temp_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "linxira-bio-output-probe-{tag}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temporary root");
        root
    }

    fn write_bytes(path: &Path, bytes: &[u8]) {
        std::fs::write(path, bytes).expect("write fixture");
    }

    fn gzip_bytes(payload: &[u8], bgzf: bool) -> Vec<u8> {
        if !bgzf {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(6));
            encoder.write_all(payload).expect("gzip payload");
            return encoder.finish().expect("finish gzip");
        }
        // A minimal hand-built BGZF block: gzip header with FEXTRA carrying
        // the `BC` subfield; only the header matters for probing.
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
        encoder.write_all(payload).expect("deflate payload");
        let compressed = encoder.finish().expect("finish deflate");
        let mut block = Vec::new();
        block.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x04]);
        block.extend_from_slice(&[0, 0, 0, 0]);
        block.extend_from_slice(&[0x00, 0xff]);
        block.extend_from_slice(&6u16.to_le_bytes());
        block.extend_from_slice(b"BC");
        block.extend_from_slice(&2u16.to_le_bytes());
        block.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        block.extend_from_slice(&compressed);
        block.extend_from_slice(&[0, 0, 0, 0]);
        block.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        block
    }

    #[test]
    fn probes_gzip_wrapped_fasta_with_a_disguised_extension() {
        let root = temp_root("gzip-fa");
        let path = root.join("sample.fa");
        write_bytes(&path, &gzip_bytes(b">seq1\nACGT\n", false));
        let result = probe_format(&path).expect("probe");
        assert_eq!(result.compression, ProbeCompression::Gzip);
        assert_eq!(result.format, BioDataFormat::Fasta);
        assert_eq!(result.confidence, ProbeConfidence::Both);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn probes_disguised_gzip_payload_over_the_name_hint() {
        let root = temp_root("gzip-bed-fq");
        let path = root.join("reads.fastq.gz");
        write_bytes(&path, &gzip_bytes(b"chr1\t0\t100\n", false));
        let result = probe_format(&path).expect("probe");
        assert_eq!(result.format, BioDataFormat::Bed);
        assert_eq!(result.compression, ProbeCompression::Gzip);
        assert_eq!(result.confidence, ProbeConfidence::Magic);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn probes_seven_zip_magic_despite_a_gz_extension() {
        let root = temp_root("7z-gz");
        let path = root.join("archive.gz");
        let mut seven_zip = SEVENZIP_MAGIC.to_vec();
        seven_zip.extend_from_slice(b"payload");
        write_bytes(&path, &seven_zip);
        let result = probe_format(&path).expect("probe");
        assert_eq!(result.compression, ProbeCompression::SevenZip);
        assert_eq!(result.confidence, ProbeConfidence::Magic);
        assert_eq!(result.format, BioDataFormat::Unknown);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn probes_bgzf_headers() {
        let head = gzip_bytes(b"ACGT", true);
        assert!(is_bgzf(&head));
        assert!(!is_bgzf(&gzip_bytes(b"ACGT", false)));
    }

    #[test]
    fn probes_disguised_fastq_as_magic_confidence() {
        let root = temp_root("fastq-fa");
        let path = root.join("reads.fa");
        write_bytes(&path, b"@r1\nACGT\n+\nIIII\n");
        let result = probe_format(&path).expect("probe");
        assert_eq!(result.format, BioDataFormat::Fastq);
        assert_eq!(result.compression, ProbeCompression::None);
        assert_eq!(result.confidence, ProbeConfidence::Magic);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn probes_plain_vcf_gff_and_bed_sniffing() {
        assert_eq!(
            sniff_content(b"##fileformat=VCFv4.2\n#CHROM\tPOS\n"),
            Some(BioDataFormat::Vcf)
        );
        assert_eq!(
            sniff_content(b"##gff-version 3\n"),
            Some(BioDataFormat::Gff3)
        );
        assert_eq!(
            sniff_content(b"chr1\t0\t100\tname\n"),
            Some(BioDataFormat::Bed)
        );
        assert_eq!(
            sniff_content(b"@HD\tVN:1.6\tSO:unsorted\n"),
            Some(BioDataFormat::Sam)
        );
        assert_eq!(
            sniff_content(b"chr1\teng\tgene\t1\t100\t.\t+\t0\t.\n"),
            Some(BioDataFormat::Gff3)
        );
    }

    #[test]
    fn probes_sra_archives_by_magic_and_extension() {
        let root = temp_root("sra");
        let by_magic = root.join("SRR000001.sra");
        write_bytes(&by_magic, b"NCBI\x01sra-payload");
        let result = probe_format(&by_magic).expect("probe");
        assert!(result.sra);

        let by_extension = root.join("SRR000002.sra");
        write_bytes(&by_extension, b"\x00\x01\x02\x03");
        assert!(probe_format(&by_extension).expect("probe").sra);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn ensure_decompressed_is_idempotent_for_plain_files() {
        let root = temp_root("plain");
        let path = root.join("sample.fa");
        write_bytes(&path, b">seq1\nACGT\n");
        let resolved = ensure_decompressed(&path, &root.join("tmp")).expect("plain passthrough");
        assert_eq!(resolved, path);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn ensure_decompressed_streams_gzip_and_preserves_bytes() {
        let root = temp_root("gunzip");
        let original = ">seq1\nACGTACGT\n>seq2\nGGTT\n".repeat(64);
        let compressed = root.join("reads.fa.gz");
        write_bytes(&compressed, &gzip_bytes(original.as_bytes(), false));
        let unpacked = ensure_decompressed(&compressed, &root.join("tmp")).expect("gunzip");
        let text = std::fs::read_to_string(&unpacked).expect("read unpacked");
        assert_eq!(text, original);
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn native_7z_lookup_reports_without_requiring_installation() {
        // CI machines may or may not ship 7-Zip; either answer is valid, but
        // the lookup must not panic and must prefer an existing binary.
        let _ = find_native_7z();
    }

    #[test]
    fn archive_extraction_requires_native_7z_or_succeeds_with_it() {
        let root = temp_root("7z-extract");
        if find_native_7z().is_none() {
            let path = root.join("archive.zip");
            write_bytes(&path, &ZIP_MAGIC);
            let error =
                ensure_decompressed(&path, &root.join("tmp")).expect_err("needs 7z installed");
            assert!(error.to_string().contains("p7zip"));
        }
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn sra_tool_probe_runs_without_sra_tools_installed() {
        let _ = probe_sra_tools();
    }

    #[test]
    fn extension_probing_walks_compression_suffixes() {
        assert_eq!(
            extension_format(Path::new("reads.fastq.gz")),
            Some(BioDataFormat::Fastq)
        );
        assert_eq!(extension_format(Path::new("archive.7z")), None);
        assert_eq!(
            extension_format(Path::new("genome.fa")),
            Some(BioDataFormat::Fasta)
        );
    }
}
