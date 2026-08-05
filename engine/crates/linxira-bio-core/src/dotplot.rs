use serde::Serialize;
use std::error::Error;
use std::fmt::Write;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct DotplotResult {
    pub query_id: String,
    pub reference_id: String,
    pub query_length: usize,
    pub reference_length: usize,
    pub kmer_size: usize,
    pub match_count: usize,
    pub window_size: usize,
    pub output_path: String,
}

#[derive(Debug, Clone)]
pub struct DotplotOptions {
    pub width: u32,
    pub height: u32,
    pub kmer_size: usize,
    pub window_size: usize,
    pub title: Option<String>,
}

impl Default for DotplotOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 800,
            kmer_size: 8,
            window_size: 5,
            title: None,
        }
    }
}

#[derive(Debug)]
pub enum DotplotError {
    Io(std::io::Error),
    InvalidInput(String),
    EmptySequence(String),
}

impl Display for DotplotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::EmptySequence(name) => write!(f, "empty sequence: {name}"),
        }
    }
}

impl Error for DotplotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DotplotError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

fn read_fasta_sequence(path: &Path) -> Result<(String, Vec<u8>), DotplotError> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut id = String::new();
    let mut sequence = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Some(stripped) = line.strip_prefix('>') {
            if id.is_empty() {
                id = stripped
                    .split_whitespace()
                    .next()
                    .unwrap_or(stripped)
                    .to_string();
            }
            continue;
        }
        for &byte in line.as_bytes() {
            match byte {
                b'A' | b'a' | b'C' | b'c' | b'G' | b'g' | b'T' | b't' | b'U' | b'u' | b'N'
                | b'n' => sequence.push(byte.to_ascii_uppercase()),
                _ => {} // skip non-DNA characters
            }
        }
    }
    if id.is_empty() {
        return Err(DotplotError::InvalidInput(
            "no FASTA header found".to_owned(),
        ));
    }
    if sequence.is_empty() {
        return Err(DotplotError::EmptySequence(id));
    }
    Ok((id, sequence))
}

fn encode_base(byte: u8) -> u64 {
    match byte {
        b'A' => 0,
        b'C' => 1,
        b'G' => 2,
        b'T' | b'U' => 3,
        _ => 4, // N or other
    }
}

fn build_kmer_index(sequence: &[u8], k: usize) -> Vec<u64> {
    if sequence.len() < k {
        return Vec::new();
    }
    let mask = (1u64 << (2 * k)) - 1;
    let mut hashes = Vec::with_capacity(sequence.len() - k + 1);
    let mut hash: u64 = 0;
    for (i, &byte) in sequence.iter().enumerate() {
        let code = encode_base(byte);
        if code > 3 {
            // N base: reset hash
            hash = 0;
            if i + k <= sequence.len() {
                hashes.push(u64::MAX); // sentinel for N-containing k-mers
            }
            continue;
        }
        hash = ((hash << 2) | code) & mask;
        if i + 1 >= k {
            hashes.push(hash);
        }
    }
    hashes
}

pub fn render_dotplot_svg_path(
    query_path: impl AsRef<Path>,
    reference_path: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &DotplotOptions,
) -> Result<DotplotResult, DotplotError> {
    let query_path = query_path.as_ref();
    let reference_path = reference_path.as_ref();
    let output = output.as_ref();

    let (query_id, query_seq) = read_fasta_sequence(query_path)?;
    let (ref_id, ref_seq) = read_fasta_sequence(reference_path)?;

    let query_kmers = build_kmer_index(&query_seq, options.kmer_size);
    let ref_kmers = build_kmer_index(&ref_seq, options.kmer_size);

    if query_kmers.is_empty() {
        return Err(DotplotError::InvalidInput(format!(
            "query sequence shorter than k-mer size ({})",
            options.kmer_size
        )));
    }
    if ref_kmers.is_empty() {
        return Err(DotplotError::InvalidInput(format!(
            "reference sequence shorter than k-mer size ({})",
            options.kmer_size
        )));
    }

    // Build reference hash map: kmer_hash -> Vec<positions>
    let mut ref_index: std::collections::HashMap<u64, Vec<usize>> =
        std::collections::HashMap::new();
    for (pos, &hash) in ref_kmers.iter().enumerate() {
        if hash != u64::MAX {
            ref_index.entry(hash).or_default().push(pos);
        }
    }

    // Find matches
    let mut match_count = 0usize;
    let query_len = query_kmers.len();
    let ref_len = ref_kmers.len();
    let max_dim = query_len.max(ref_len).max(1);

    // Use a sparse match representation
    let mut matches: Vec<(usize, usize)> = Vec::new();

    for (qpos, &hash) in query_kmers.iter().enumerate() {
        if hash == u64::MAX {
            continue;
        }
        if let Some(positions) = ref_index.get(&hash) {
            for &rpos in positions {
                matches.push((qpos, rpos));
                match_count += 1;
                if match_count > 500_000 {
                    // Cap matches to avoid huge SVGs
                    break;
                }
            }
        }
        if match_count > 500_000 {
            break;
        }
    }

    let margin = 60u32;
    let label_width = 200u32;
    let plot_x = margin + label_width;
    let plot_y = margin;
    let plot_width = options.width.saturating_sub(plot_x + margin);
    let plot_height = options.height.saturating_sub(plot_y + margin + label_width);

    let x_scale = plot_width as f64 / max_dim as f64;
    let y_scale = plot_height as f64 / max_dim as f64;

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
        options.width, options.height, options.width, options.height
    );
    let _ = writeln!(svg, r#"<rect width="100%" height="100%" fill="white"/>"#);

    // Title
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| format!("{query_id} vs {ref_id}"));
    let _ = writeln!(
        svg,
        r#"<text x="{}" y="30" font-family="monospace" font-size="16" text-anchor="middle" fill="black">{}</text>"#,
        options.width / 2,
        escape_xml(&title)
    );

    // Axis labels
    let _ = writeln!(
        svg,
        r#"<text x="{}" y="{}" font-family="monospace" font-size="12" text-anchor="middle" fill="black" transform="rotate(-90, {}, {})">Query: {} ({})</text>"#,
        15,
        plot_y + plot_height / 2,
        15,
        plot_y + plot_height / 2,
        escape_xml(&query_id),
        query_seq.len()
    );
    let _ = writeln!(
        svg,
        r#"<text x="{}" y="{}" font-family="monospace" font-size="12" text-anchor="middle" fill="black">Reference: {} ({})</text>"#,
        plot_x + plot_width / 2,
        options.height - 15,
        escape_xml(&ref_id),
        ref_seq.len()
    );

    // Plot area background
    let _ = write!(
        svg,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f8f8f8\" stroke=\"black\" stroke-width=\"1\"/>",
        plot_x, plot_y, plot_width, plot_height
    );

    // Draw matches as points
    if match_count <= 500_000 {
        for &(qpos, rpos) in &matches {
            let x = plot_x as f64 + (rpos as f64 * x_scale);
            let y = plot_y as f64 + (qpos as f64 * y_scale);
            let _ = writeln!(
                svg,
                r#"<circle cx="{:.1}" cy="{:.1}" r="0.5" fill="black" opacity="0.3"/>"#,
                x, y
            );
        }
    }

    // Diagonal line
    let _ = writeln!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="red" stroke-width="0.5" stroke-dasharray="4,4" opacity="0.5"/>"#,
        plot_x,
        plot_y,
        plot_x + plot_width,
        plot_y + plot_height
    );

    let _ = writeln!(svg, "</svg>");

    fs::write(output, svg)?;

    Ok(DotplotResult {
        query_id,
        reference_id: ref_id,
        query_length: query_seq.len(),
        reference_length: ref_seq.len(),
        kmer_size: options.kmer_size,
        match_count,
        window_size: options.window_size,
        output_path: output.display().to_string(),
    })
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_dna_bases() {
        assert_eq!(encode_base(b'A'), 0);
        assert_eq!(encode_base(b'C'), 1);
        assert_eq!(encode_base(b'G'), 2);
        assert_eq!(encode_base(b'T'), 3);
        assert_eq!(encode_base(b'N'), 4);
    }

    #[test]
    fn builds_kmer_index_for_simple_sequence() {
        let seq = b"AAAA";
        let hashes = build_kmer_index(seq, 2);
        assert_eq!(hashes.len(), 3);
        assert_eq!(hashes, vec![0, 0, 0]);
    }

    #[test]
    fn different_kmers_have_different_hashes() {
        let seq = b"ACGT";
        let hashes = build_kmer_index(seq, 2);
        assert_eq!(hashes.len(), 3);
        assert_ne!(hashes[0], hashes[1]);
        assert_ne!(hashes[1], hashes[2]);
    }

    #[test]
    fn empty_sequence_returns_error() {
        let dir = std::env::temp_dir().join(format!("dotplot-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("empty.fa");
        fs::write(&path, ">empty\n").unwrap();
        let result = read_fasta_sequence(&path);
        let _ = fs::remove_dir_all(&dir);
        assert!(result.is_err());
    }
}
