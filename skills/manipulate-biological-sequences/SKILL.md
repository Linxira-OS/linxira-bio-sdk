---
name: manipulate-biological-sequences
description: Run verified local Linxira Bio FASTA sequence-manipulation and analysis capabilities. Use for FASTA extraction, filtering, reverse complements, translation, ORFs, ID normalization, merge/split, FASTA-table conversion, exact k-mer counting, consensus-from-alignment, sequence-order shuffling, Biopython-backed sequence format conversion, and simple exact-match electronic PCR.
---

# Manipulate Biological Sequences

Use these local capabilities instead of writing ad-hoc FASTA scripts.

## Choose The Capability

- Use `sequence.extract.v1` for full records by `--id ID` or regions with
  `--region ID:START-END[:+|-]`.
- Use `sequence.filter.v1` for record filters by length, GC percentage, or N
  percentage.
- Use `sequence.reverse-complement.v1` for DNA or RNA reverse complements.
- Use `sequence.translate.v1` for nucleotide FASTA translation with the NCBI
  standard genetic code.
- Use `sequence.orf.v1` for deterministic ATG-to-stop ORF discovery.
- Use `sequence.id.normalize.v1` to rewrite FASTA identifiers with deterministic
  prefixes, numeric indexes, optional zero-padding, and optional description
  preservation.
- Use `sequence.merge.v1` to concatenate one or more FASTA files while rejecting
  duplicate identifiers unless explicitly allowed.
- Use `sequence.split.v1` to split one FASTA into deterministic numbered chunk
  files.
- Use `sequence.to-table.v1` to export FASTA records to CSV/TSV columns
  `id`, `description`, `length`, and `sequence`.
- Use `sequence.from-table.v1` to rebuild FASTA from CSV/TSV columns.
- Use `sequence.kmer.count.v1` for exact k-mer counts with optional canonical reverse-complement collapsing.
- Use `sequence.consensus.v1` to compute a majority-rule consensus from a multiple sequence alignment FASTA.
- Use `sequence.shuffle.v1` to randomize the order of FASTA records with a reproducible seed.
- Use `sequence.convert.biopython.v1` to convert between FASTA, FASTQ, GenBank, and EMBL via the locked Biopython workflow pack (requires the project Python 3.12 runtime).
- Use `primer.epcr.v1` for exact-match primer pairs supplied as TSV columns `id`, `forward`, and `reverse`.

Run `inspect-bio-dataset` first when the input format or compression is not
already known.

## Run

Examples:

```bash
linxira-bio sequence extract INPUT.fa OUTPUT.fa --id chr1 --region chr2:100-250:- --strict --json
linxira-bio sequence filter INPUT.fa OUTPUT.fa --min-length 1000 --max-n-percent 5 --json
linxira-bio sequence reverse-complement INPUT.fa OUTPUT.fa --json
linxira-bio sequence translate INPUT.fa OUTPUT.fa --frame 1 --trim-terminal-stop --json
linxira-bio sequence orf INPUT.fa OUTPUT.faa --min-amino-acids 30 --include-partial-3prime --json
linxira-bio sequence normalize-ids INPUT.fa OUTPUT.fa --prefix seq --width 6 --json
linxira-bio sequence merge merged.fa part1.fa part2.fa --json
linxira-bio sequence split INPUT.fa split-dir --records-per-file 1000 --json
linxira-bio sequence to-table INPUT.fa records.tsv --delimiter tsv --json
linxira-bio sequence from-table records.tsv OUTPUT.fa --delimiter tsv --json
linxira-bio sequence kmer-count INPUT.fa kmers.tsv --k 21 --canonical --top-n 50 --json
linxira-bio sequence consensus alignment.fa consensus.fa --threshold 0.5 --json
linxira-bio sequence shuffle INPUT.fa OUTPUT.fa --seed 42 --json
linxira-bio sequence convert INPUT.fasta OUTPUT.genbank --output-format genbank
linxira-bio primer epcr reference.fa primers.tsv amplicons.tsv --max-amplicon 5000 --json
```

When developing inside the source repository, prefix commands with
`cargo run -p linxira-bio-cli --`.

## Validate

- Preserve the capability ID and JSON result envelope.
- Treat any non-zero exit or `status: error` worker result as failure.
- Confirm the output path is new; these capabilities refuse to overwrite files.
- For coordinate extraction, report that coordinates are 1-based inclusive.
- For reverse-complement and translation, reject mixed T/U records rather than
  interpreting them silently.
- Do not use ORF output as a gene-prediction claim without annotation evidence.
- For merge, keep duplicate IDs rejected by default unless the user explicitly
  chooses `--allow-duplicate-ids`.
- For split, treat the output directory as an artifact directory and never reuse
  pre-existing chunk filenames.
- For table conversion, preserve the `id`, `description`, `length`, and
  `sequence` column contract unless the user explicitly maps input columns.
- For k-mer counts, report skipped ambiguous windows and do not interpret the
  spectrum as a genome-size or error estimate without a separate model.
- For ePCR, report exact-match and 1-based inclusive coordinate limitations;
  do not claim experimental amplification success.
- For consensus, verify that all input sequences have the same ungapped length
  and report the threshold used; single-sequence inputs produce a warning but
  are not rejected.
- For shuffle, verify the output contains the same number of sequences and
  total residues as the input; the seed guarantees reproducibility.
- For convert, confirm the input and output formats are one of FASTA, FASTQ,
  GenBank, or EMBL; FASTA-to-FASTQ conversion without quality scores is not
  supported, and sequence counts must match between input and output.
