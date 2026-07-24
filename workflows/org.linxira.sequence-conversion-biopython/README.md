# Biopython sequence conversion workflow pack

This first-party pack converts uncompressed FASTA, FASTQ, GenBank, and EMBL
files through Biopython `SeqIO`. It executes locally, validates the complete
artifact-aware request, and verifies declared input size and optional SHA-256.
The converted file and `result.json` are built in a private sibling directory
and activated together with one same-filesystem directory rename.

The source is cataloged for review but is not installable or dispatched by the
application. `environment.apply.v1`, signed artifact resolution, and the
workflow executor must be implemented before that status can change.

Run inside the exact environment described by `requirements.lock`:

```text
python src/convert_sequences.py --request request.json --result output/result.json
```

The request and result contracts are in `schemas/`. Format conversion can lose
annotations that the destination representation cannot express. In
particular, FASTA contains identifiers and sequence only; FASTQ output requires
quality annotations, and GenBank output may require a molecule type annotation.
The output filename must be one portable path component: ASCII control
characters, Win32-reserved characters (including the ADS separator `:`),
trailing spaces or dots, `result.json`, and Windows device names are rejected
even when the pack is reviewed on POSIX. No input is uploaded and no network
access is requested during execution.
