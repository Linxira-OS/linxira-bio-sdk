# FASTA Sequence Statistics

## Purpose

Compute FASTA record count, lengths, N50/L50, auN, GC percentage, and N content locally.

## Inputs

One readable FASTA file. Multiline sequences are supported and headers must start with `>`.

## Parameters

- `<input.fasta[.gz]>` (required): the FASTA file, optionally gzip-compressed
  (detected by magic bytes).
- `--backend auto|rust|python|r`: the implementation backend. `rust` (and
  omission) runs the native engine in-process; `python` and `r` route through
  the worker to the `org.linxira.benchmark-python` / `org.linxira.benchmark-r`
  packs (Biopython / Biostrings implementations) and produce the V2 result
  envelope. `auto` consults `runtime-preferences.json`: a miss keeps `rust`,
  and a non-rust hit emits a `backend_from_preferences` warning.
- `--json`: print the full result envelope instead of the field list.

## Outputs

Returns `sequence_count`, `total_bases`, minimum, maximum, and mean lengths,
`n50`, `l50`, `au_n`, `gc_percent`, `n_count`, and `n_percent`.

## Examples

```bash
linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json
```

## Interpretation

N50 is the sequence length at half of total length; L50 is the number of
sequences needed to reach that threshold. They describe contiguity, not assembly correctness.

## Caveats

GC percentage uses only A/C/G/T as its denominator. N percentage uses all
sequence characters. These statistics do not correct contamination, ploidy, or assembly errors.

## Runtime Dependencies

- `rust` (default): a pure local Rust capability with no external tools.
- `python` / `r`: the matching benchmark pack and its interpreter plus
  packages — Python with `biopython`, or R with `Biostrings`, `jsonlite`,
  and `digest`. No network access in either case.

## Citations

N50/L50 use their conventional definitions. auN is the length-weighted mean
`sum(length^2) / sum(length)`.

## Troubleshooting

- If sequence data is reported before the first header, confirm the file is
  FASTA and remove non-header content at its beginning.
- `dependency Biostrings ... is not installed`: the R pack resolves packages
  from the host R installation; install the package or point
  `LINXIRA_BIO_WORKFLOW_R_LIBRARY` at a project library that has it.
- `unknown --backend value`: the flag accepts `auto`, `rust`, `python`, and
  `r` only.
