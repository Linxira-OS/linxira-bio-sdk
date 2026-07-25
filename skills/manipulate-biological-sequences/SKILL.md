---
name: manipulate-biological-sequences
description: Run verified local Linxira Bio FASTA sequence-manipulation capabilities. Use for FASTA record or coordinate extraction, length/GC/N filtering, DNA/RNA reverse-complement generation, nucleotide-to-protein translation, and deterministic ORF finding.
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
