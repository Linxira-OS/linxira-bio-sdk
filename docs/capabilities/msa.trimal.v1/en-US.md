# trimAl Alignment Trimming

## Purpose

Trim a local multiple-sequence alignment with a maintained trimAl heuristic.

## Inputs

Provide a local FASTA multiple-sequence alignment.

## Parameters

Choose `automated1`, `gappyout`, `strict`, `strictplus`, or `nogaps`.

## Outputs

The capability writes a new FASTA alignment and JSON execution metadata. It
refuses to overwrite the input or an existing output.

```bash
linxira-bio msa trimal alignment.fa trimmed.fa --mode automated1 --json
```

## Examples

The command above runs the default automated heuristic and writes a distinct
alignment artifact.

## Interpretation

Compare retained alignment length and taxon coverage before tree inference.

## Caveats

Trimming changes the analyzed character set. Record the mode, inspect retained
columns, and do not treat a trimmed alignment as automatically superior.

## Runtime Dependencies

Requires trimAl on `PATH` or `LINXIRA_BIO_TRIMAL`. The executable is invoked
directly without a shell.

## Citations

Cite trimAl, its version, selected heuristic, and the original alignment method.

## Troubleshooting

Audit `trimal`; configure `LINXIRA_BIO_TRIMAL` only when it is outside `PATH`.
