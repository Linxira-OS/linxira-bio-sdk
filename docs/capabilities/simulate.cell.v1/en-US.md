# Cell Lineage Simulation

## Purpose

Simulate a mechanistic single-cell lineage with seeded, exact stochastic
kinetics — telegraph (two-state promoter) gene expression, Monod-coupled
growth, and binomial division — as deterministic synthetic ground truth for
benchmark runs, method validation, and GPU consistency references.

## Inputs

Optional model JSON. Without it, a built-in 12-gene panel runs with
Monod-coupled biomass growth and substrate exhaustion on a default two-hour
lineage.

## Parameters

`--model model.json` overrides the model: `monod_constant`,
`substrate_initial`, `division_biomass`, and `genes[]` carrying per-minute
`kon` (promoter ON), `koff` (promoter OFF), `ks` (transcription), `kdm`
(mRNA decay), `ktl` (translation per mRNA), `kdp` (protein decay), and
`yield_per_protein` (biomass units per synthesized protein). `--time`
sets simulated minutes (default 120), `--seed` the generator seed
(default 42), `--bins` the time-series bin count (default 100, range
1-100000), and `--max-events` the Gillespie event budget (default
20,000,000).

## Outputs

A TSV time series (binned means of total mRNA, total protein, biomass, the
Monod factor, and divisions per bin) and a JSON summary: seed, model echo,
event/transcription/translation counters, division count, lineage depth,
substrate trajectory, final biomass, mean generation time, and per-gene
time-averaged and final expression.

## Examples

```bash
linxira-bio simulate cell lineage.tsv --time 120 --seed 42 --json
linxira-bio simulate cell lineage.tsv --model mymodel.json --time 600 --bins 200 --seed 7 --json
```

## Interpretation

The Monod factor tracks medium exhaustion: it stays near 1 while substrate
is abundant, collapses as the lineage depletes it, and growth arrests in
stationary phase. Mean generation time covers the dividing window only
(first to last division). Per-gene means report time-averaged mRNA/protein
occupancy; burstiness comes from the telegraph promoter, so identical genes
diverge. Seed plus model plus options is the reproducibility contract:
identical inputs regenerate the TSV and summary byte for byte on any
machine — cross-machine byte parity is the acceptance gate.

## Caveats

The model is mechanistic, not a fitted organism: one compartment, no
volume, no DNA replication, protein decay does not shrink biomass, and
division splits the tracked daughter binomially while the sister lineage is
discarded. Rate constants are illustrative, not measured; do not compare
outputs to measurements of a real strain. f64 event ordering is
IEEE-deterministic, so parity holds across platforms for this engine.

## Runtime Dependencies

None beyond the CLI — pure Rust, exact Gillespie direct method over the
xoshiro256** stream seeded via SplitMix64.

## Citations

Method lineage: Gillespie, D. T. *J. Comput. Chem.* **1977**, 28(8),
2347-2357 (exact stochastic simulation); the two-state promoter follows the
standard telegraph model; Monod growth follows Monod, A. *Annu. Rev.
Microbiol.* **1949**, 3, 371-394. Cite this capability by version when
simulated lineages underpin a published comparison.

## Troubleshooting

No divisions: raise `--time`, lower `division_biomass`, or raise translation
`ktl`/`yield_per_protein` in the model. Event budget exhausted: raise
`--max-events`, shorten `--time`, or reduce the gene count. Output differs
between runs or machines: the seed, model file, or an option differs —
diff the JSON summary `model` echo against the ledger entry.
