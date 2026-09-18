# r=1.000000 live re-verification (SRR1460477, 2026-09-18)

Independent re-verification of the blog's headline claim (TPM Pearson
r = 1.000000 vs the reference pipeline's stored quant.sf), run on the
benchmark host with a fresh SRA pull, fresh fasterq-dump dump, and the
SDK orchestrating salmon 2.7.0 with matched parameters
(`-l A -p 8 --validateMappings --seqBias --gcBias`).

## Result

| metric | value |
|---|---|
| transcripts compared | 33,955 |
| TPM Pearson r | 1.000000000 |
| max \|ΔTPM\| | 0 |
| NumReads identical | 33,955 / 33,955 |
| SHA256(sdk_quant_bias.sf) | 9b6a4cca0461e8d4117c88dc996f23f83a1730e6352936f82714377e691536af |
| SHA256(reference quant.sf, host-side) | 9b6a4cca0461e8d4117c88dc996f23f83a1730e6352936f82714377e691536af |

The two files are byte-identical: the SDK-orchestrated rerun and the
reference pipeline produced the same 1,503,599-byte quant.sf.

## Control experiment (why r=1 is earned, not trivial)

Same run, same everything, minus `--seqBias --gcBias`
(`control_firstpass_nobias.sf`, the retained first-pass output):

| metric | value |
|---|---|
| TPM Pearson r | 0.980896 |
| max \|ΔTPM\| | 1.27e+04 |
| NumReads identical | 25,422 / 33,955 (75%) |

One missing parameter pair drops r from 1.000000 to 0.9809 — the
comparison is sensitive, and the reported identity requires exact
parameter match. r = 1 is only expected for "same deterministic tool,
same parameters, same input, re-run"; any cross-implementation or
cross-parameter comparison must yield r < 1.

## Files

- `sdk_quant_bias.sf` — SDK-orchestrated quant.sf (bias flags, this rerun)
- `control_firstpass_nobias.sf` — retained first-pass quant.sf (no bias flags)
- `live_rerun_output.txt` — verbatim compare output (LIVE_R / MAX_ABS_DTPM /
  NUMREADS_IDENTICAL)
- `reproduce.sh` — the exact reproduction script (pull SRA from staging,
  fasterq-dump, gzip, SDK quantify, compare)
- `SHA256SUMS.txt` — hashes of all of the above

## Reproduce

Input reads: NCBI SRA SRR1460477 (public). Index: Pinku1 CDS salmon index
(salmon >= 2.7.0; older salmon rejects the v2 index). Then run
`reproduce.sh` steps 1-4 and compare against the reference quant.sf for
the same run; with matched parameters the output must be byte-identical
(same SHA256 as `sdk_quant_bias.sf`).
