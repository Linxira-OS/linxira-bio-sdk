---
name: analyze-sequence-statistics
description: Compute and validate deterministic FASTA sequence or assembly metrics with the local Linxira Bio `sequence.stats.v1` capability. Use for FASTA record counts, total and mean length, minimum and maximum length, N50/L50, auN, GC percentage, and ambiguous-N summaries.
---

# Analyze Sequence Statistics

Use the tested local capability instead of writing a one-off FASTA parser.

## Run

1. Confirm with `inspect-bio-dataset` that the input is a supported local FASTA.
   Plain text, gzip, and BGZF streams are accepted.
2. Confirm that the decompressed payload starts with a non-empty `>` header and
   is appropriate for aggregate sequence statistics.
3. Run the installed CLI:

```bash
linxira-bio sequence stats INPUT.fasta --json
```

When developing inside the source repository, run:

```bash
cargo run -p linxira-bio-cli -- sequence stats INPUT.fasta --json
```

4. Preserve the `sequence.stats.v1` capability ID and JSON result with the input
   hash and CLI version.
5. Reject an error result; do not interpret partial terminal output as a valid
   analysis.

## Interpret

- `sequence_count` is the number of FASTA records, including zero-length
  records.
- `total_bases` counts non-whitespace sequence characters.
- `gc_percent` uses canonical `A`, `C`, `G`, `T`, and `U` as its denominator;
  other IUPAC ambiguity symbols are excluded.
- `n_percent` uses all counted sequence characters as its denominator.
- `n50` is the shortest sequence in the length-ranked prefix covering at least
  half of all bases; `l50` is that prefix size.
- `au_n` is the length-weighted mean sequence length.

Do not compare N50 across assemblies with materially different expected genome
size, contamination, haplotig handling, or filtering without noting those
differences. These metrics describe sequence content; they do not establish
assembly correctness or biological completeness.
