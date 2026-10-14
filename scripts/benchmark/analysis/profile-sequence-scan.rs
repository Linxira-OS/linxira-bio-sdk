// ---------------------------------------------------------------------------
// Benchmark tooling — scripts/benchmark/analysis/profile-sequence-scan.rs
// Role: standalone micro-probe that splits a FASTA scan into I/O, per-line overhead and per-base classification, so a slow capability can be attributed instead of guessed at
// Windows: 2026-10-13 window (post-hoc analysis of its rust leg, recorded in docs/engine-evals/sequence-stats-branch-cost-2026-10-14.md)
// Direction: read-only over one input file; prints timings to stdout; never writes beside the input
// Provenance index: scripts/benchmark/README.md
// ---------------------------------------------------------------------------
// Standard library only, on purpose: no cargo project, no repository build, no
// dependencies — the thing under test is a loop, and the point is that anyone can
// re-run this on any machine in one command:
//
//     rustc -C opt-level=3 scripts/benchmark/analysis/profile-sequence-scan.rs -o /tmp/scan
//     /tmp/scan /path/to/big.fa
//
// The four arms answer different questions:
//   read-only         how much is the filesystem (1 MiB chunks, no interpretation)
//   lines + branchy   the shape `fasta_stats` used before 2026-10-14: a `match` per base
//   lines + table     the same reader with a 256-byte class table and no per-base branch
//   chunks + table    raw chunked reading with the same table
// On random sequence the branchy arm costs several times the table arm while doing the
// same arithmetic; on an all-A or ACGT-cyclic file built to the same size (within 441 bytes)
// they converge, which
// is what identifies branch misprediction rather than work. All counting arms must agree,
// which the program checks and prints as `agreement:`.
//
// Every arm is run three times and the fastest is reported, because a single reading on a
// shared machine is not evidence.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::time::Instant;

/// Bit 0 canonical (A/T/U/G/C), bit 1 G/C, bit 2 N, bit 3 counted as a sequence byte
/// (non-whitespace). Built with `to_ascii_uppercase`, the primitive the per-base `match`
/// used, so the table is case-equivalent by construction.
const CLASS: [u8; 256] = {
    let mut table = [0_u8; 256];
    let mut byte = 0_usize;
    while byte < 256 {
        let value = byte as u8;
        if !value.is_ascii_whitespace() {
            let upper = value.to_ascii_uppercase();
            let mut class = 8_u8;
            if matches!(upper, b'A' | b'T' | b'U' | b'G' | b'C') {
                class |= 1;
            }
            if matches!(upper, b'G' | b'C') {
                class |= 2;
            }
            if upper == b'N' {
                class |= 4;
            }
            table[byte] = class;
        }
        byte += 1;
    }
    table
};

type Counts = (u64, u64, u64, u64);

fn lines_branchy(path: &str) -> Counts {
    let mut reader = BufReader::new(File::open(path).expect("open"));
    let mut line = String::new();
    let (mut length, mut canonical, mut gc, mut n) = (0_u64, 0_u64, 0_u64, 0_u64);
    loop {
        line.clear();
        if reader.read_line(&mut line).expect("read") == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('>') {
            continue;
        }
        for base in trimmed.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            length += 1;
            match base.to_ascii_uppercase() {
                b'G' | b'C' => {
                    gc += 1;
                    canonical += 1;
                }
                b'A' | b'T' | b'U' => canonical += 1,
                b'N' => n += 1,
                _ => {}
            }
        }
    }
    (length, canonical, gc, n)
}

fn lines_table(path: &str) -> Counts {
    let mut reader = BufReader::new(File::open(path).expect("open"));
    let mut line = String::new();
    let (mut length, mut canonical, mut gc, mut n) = (0_u64, 0_u64, 0_u64, 0_u64);
    loop {
        line.clear();
        if reader.read_line(&mut line).expect("read") == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('>') {
            continue;
        }
        for &base in trimmed.as_bytes() {
            let class = CLASS[base as usize];
            length += u64::from(class >> 3 & 1);
            canonical += u64::from(class & 1);
            gc += u64::from(class >> 1 & 1);
            n += u64::from(class >> 2 & 1);
        }
    }
    (length, canonical, gc, n)
}

fn chunks_table(path: &str) -> Counts {
    let mut file = File::open(path).expect("open");
    let mut buffer = vec![0_u8; 1 << 20];
    let (mut length, mut canonical, mut gc, mut n) = (0_u64, 0_u64, 0_u64, 0_u64);
    let mut in_header = false;
    loop {
        let read = file.read(&mut buffer).expect("read");
        if read == 0 {
            break;
        }
        for &base in &buffer[..read] {
            if base == b'>' {
                in_header = true;
                continue;
            }
            if base == b'\n' {
                in_header = false;
                continue;
            }
            if in_header {
                continue;
            }
            let class = CLASS[base as usize];
            length += u64::from(class >> 3 & 1);
            canonical += u64::from(class & 1);
            gc += u64::from(class >> 1 & 1);
            n += u64::from(class >> 2 & 1);
        }
    }
    (length, canonical, gc, n)
}

fn fastest(name: &str, scan: fn(&str) -> Counts, path: &str) -> Counts {
    let mut best = f64::MAX;
    let mut counts = (0, 0, 0, 0);
    for _ in 0..3 {
        let started = Instant::now();
        counts = scan(path);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        if elapsed < best {
            best = elapsed;
        }
    }
    println!(
        "  {name:<26} {best:>8.1} ms   (length {}, canonical {}, gc {}, n {})",
        counts.0, counts.1, counts.2, counts.3
    );
    counts
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: scan <fasta>");
    let bytes = std::fs::metadata(&path).expect("stat").len();
    println!("file {path}  ({bytes} bytes)");

    let mut read_only_best = f64::MAX;
    for _ in 0..3 {
        let started = Instant::now();
        {
            let mut file = File::open(&path).expect("open");
            let mut buffer = vec![0_u8; 1 << 20];
            while file.read(&mut buffer).expect("read") != 0 {}
        }
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        if elapsed < read_only_best {
            read_only_best = elapsed;
        }
    }
    println!("  {:<26} {:>8.1} ms", "read-only 1 MiB chunks", read_only_best);

    let branchy = fastest("lines + branchy match", lines_branchy, &path);
    let table = fastest("lines + class table", lines_table, &path);
    let chunked = fastest("1 MiB chunks + class table", chunks_table, &path);
    println!("  agreement: {}", branchy == table && table == chunked);
}
