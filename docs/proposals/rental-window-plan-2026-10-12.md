# Rental window plan: the GIAB correctness gate, and why the tool arms stay home

2026-10-12. Status: awaiting the maintainer's budget approval. Nothing is provisioned, no
cost is incurred, and no provider account is touched until that approval exists (repository
rule: cloud resources require explicit approval).

## What the window is for

One thing: a **correctness** gate against a third-party truth set — GIAB HG002 small
variants, compared with `hap.py`. Every published performance number so far comes from our
own harness, and the parity arms only prove that three implementations of the *same*
algorithm agree. Neither says the calls are *right*. That is what this window buys.

## What the window is not for

Timing. Rental containers ship without `/usr/bin/time` (documented in
`scripts/benchmark/rental/README.md` from the 2026-10-02 rounds), so peak RSS and the disk
columns would be missing, and a rented CPU shares nothing with the pinned measurement host.
Every timing number — parity arms and the remaining tool arms (seqkit, fastp, scikit-learn)
— stays on the pinned 2-core/4-thread host, where the archived baselines and the 10 ms clock
semantics live. Mixing hosts inside one comparison would be the same error as mixing arm
classes inside one table.

## Scope: chromosome 20, not the whole genome

| Choice | Effect |
| --- | --- |
| Whole genome (~3.1 Gb), 30× | ~1 day of wall clock on 32+ cores, ≥ 250 GB of disk churn, and the result is still one number |
| **chr20 only (~64 Mb), 30×** | hours on 8–16 cores, ~10 GB of intermediates, same *kind* of evidence (precision/recall against the NIST high-confidence regions) |

The window runs the chromosome-scoped gate and every claim says "chr20". If the numbers are
interesting enough to want whole-genome, that is a second, separate decision with its own
estimate.

## Pinned inputs (recorded before the run, verified after)

| Input | Source | Note |
| --- | --- | --- |
| Reference | GRCh38, chromosome 20 (plus its `.fai`) | pinned by checksum in the window record |
| Truth set | GIAB `NISTv4.2.1` small-variant benchmark for HG002, from NCBI `ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab/release/AshkenazimTrio/HG002_NA24385_son/NISTv4.2.1/GRCh38/` | public, no egress cost; the newer v5.0q release exists and is **not** used here — one benchmark version per window, named in the report |
| High-confidence regions (BED) | same GIAB release directory | restricts comparison to regions where the truth set is defined |
| Reads | HG002 Illumina, subset for chr20 at ~30× | the provisioning script resolves and records the accession and its checksum; no accession is asserted here because it is pinned at provisioning time, not from memory |

## Pipeline the window exercises

```
align (bwa-mem2 or minimap2)  ->  variant.call.v1 (bcftools)  ->  bcftools norm  ->  hap.py
```

The point is not only the caller's accuracy but that *our* capability plumbing can express
the whole chain and be checked by a tool we did not write. `hap.py` runs from its GA4GH
container image, so its own version is recorded too.

## Deliverable and disclosure

`benchmark-results/<date>/giab-correctness/` containing: the window record (machine,
versions, every command, input checksums), `hap.py` output (precision/recall for SNP and
INDEL, within the high-confidence regions), the intermediate variant VCF, and a short README
stating the scope (chr20, benchmark version, read subset) and what would have falsified the
result. Numbers are published even when they disappoint; a gate that is only reported when
it passes is not a gate.

## Budget estimate (list prices checked 2026-10-12)

| Option | Machine | List price | 24 h | Verdict |
| --- | --- | --- | --- | --- |
| Hetzner CCX33 | 8 dedicated vCPU, 31 GiB, 240 GB NVMe | $0.252/h | ≈ $6 | sufficient if the read subset stays small |
| Hetzner CCX43-class | 16 dedicated vCPU, ~61 GiB | ≈ $0.50/h | ≈ $12 | **recommended**: comfortable for chr20 at 30× |
| Hetzner CCX53-class | 32 dedicated vCPU, ~123 GiB | ≈ $1.00/h | ≈ $24 | headroom for whole-genome curiosity, not needed |
| Hetzner AX-line dedicated | ~6–8 cores, 64 GB, bare metal | ≈ €36.70/month (≈ €1.2/day) | ≈ €1.2 | cheapest, but bare-metal ordering and a monthly billing cycle make it awkward for a one-day window |

Recommended: **one 24-hour window on a 16-vCPU dedicated-cloud class machine, budget cap
$25**, including adapter time. Traffic to NCBI is free and the provider's included traffic
(20 TB) covers everything else; no GPU, no spot preemption needed. Prices rose 20–30% on
2026-04-01 and must be re-checked at booking — the cap is the number to approve, not the
instance name.

## Risks and their handling

| Risk | Handling |
| --- | --- |
| The window overruns (data fetch slower than expected) | cap at 24 h; the pipeline is stage-checkpointed so a partial run still yields a documented result rather than nothing |
| `hap.py` container cannot start in the rental environment | fall back to `vcfeval` from the same image; if neither runs, the window reports what blocked it instead of substituting a hand-written comparison |
| chr20 result is read as a whole-genome claim | scope is printed in the report title, the README and every place the number is cited |
| Cost approval is mistaken for approval to also run timing | the plan states the opposite: timing stays on the pinned host |

## Prerequisites already in place

- `scripts/benchmark/rental/prepare-bundle.py` builds the transferable bundle from the
  2026-10-02 rounds; the provisioning script for this window reuses that pattern.
- The truth-set, region and reference URLs above are verified (NCBI GIAB release tree).
- Nothing in the plan depends on the GPU vendors or on the M5 work.
