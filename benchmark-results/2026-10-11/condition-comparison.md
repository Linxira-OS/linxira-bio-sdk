# 有并发批 (CI 抢核) (A) vs 干净批 (CI 空闲) (B)

- rows compared: 30; shifted beyond noise: 0

| capability | backend | A median ms | B median ms | Δ | noise band | verdict |
| --- | --- | --- | --- | --- | --- | --- |
| `enrichment.overrepresentation.v1` | rust | 30.000 | 30.000 | +0.00% | ±0.00% | within-noise |
| `enrichment.overrepresentation.v1` | python | 230.000 | 230.000 | +0.00% | ±8.27% | within-noise |
| `enrichment.overrepresentation.v1` | r | 4470.000 | 4480.000 | +0.22% | ±22.68% | within-noise |
| `expression.pca.v1` | rust | 20.000 | 20.000 | +0.00% | ±81.36% | within-noise |
| `expression.pca.v1` | python | 4010.000 | 4010.000 | +0.00% | ±22.89% | within-noise |
| `expression.pca.v1` | r | 2760.000 | 2770.000 | +0.36% | ±28.82% | within-noise |
| `fastq.qc.v1` | rust | 1570.000 | 1570.000 | +0.00% | ±9.26% | within-noise |
| `fastq.qc.v1` | python | 49320.000 | 49345.000 | +0.05% | ±4.88% | within-noise |
| `fastq.qc.v1` | r | 702655.000 | 702190.000 | -0.07% | ±1.92% | within-noise |
| `sequence.stats.v1` | rust | 820.000 | 810.000 | -1.22% | ±4.67% | within-noise |
| `sequence.stats.v1` | python | 840.000 | 825.000 | -1.79% | ±10.47% | within-noise |
| `sequence.stats.v1` | r | 1840.000 | 1815.000 | -1.36% | ±20.28% | within-noise |
| `sequence.stats.v1#granularity-many` | rust | 830.000 | 820.000 | -1.20% | ±2.55% | within-noise |
| `sequence.stats.v1#granularity-many` | python | 860.000 | 860.000 | +0.00% | ±2.69% | within-noise |
| `sequence.stats.v1#granularity-many` | r | 1940.000 | 1930.000 | -0.52% | ±14.69% | within-noise |
| `sequence.stats.v1#granularity-one` | rust | 820.000 | 810.000 | -1.22% | ±12.07% | within-noise |
| `sequence.stats.v1#granularity-one` | python | 850.000 | 850.000 | +0.00% | ±2.62% | within-noise |
| `sequence.stats.v1#granularity-one` | r | 1825.000 | 1820.000 | -0.27% | ±21.46% | within-noise |
| `sequence.stats.v1#granularity-twenty` | rust | 820.000 | 810.000 | -1.22% | ±2.04% | within-noise |
| `sequence.stats.v1#granularity-twenty` | python | 840.000 | 835.000 | -0.60% | ±21.10% | within-noise |
| `sequence.stats.v1#granularity-twenty` | r | 1830.000 | 1820.000 | -0.55% | ±15.08% | within-noise |
| `set.venn.v1` | rust | 10.000 | 10.000 | +0.00% | ±0.00% | within-noise |
| `set.venn.v1` | python | 180.000 | 180.000 | +0.00% | ±10.56% | within-noise |
| `set.venn.v1` | r | 10975.000 | 11075.000 | +0.91% | ±6.65% | within-noise |
| `structure.pdb.summary.v1` | rust | 30.000 | 30.000 | +0.00% | ±26.35% | within-noise |
| `structure.pdb.summary.v1` | python | 790.000 | 780.000 | -1.27% | ±33.59% | within-noise |
| `structure.pdb.summary.v1` | r | 20750.000 | 20680.000 | -0.34% | ±5.09% | within-noise |
| `variant.stats.v1` | rust | 80.000 | 80.000 | +0.00% | ±0.00% | within-noise |
| `variant.stats.v1` | python | 760.000 | 760.000 | +0.00% | ±3.34% | within-noise |
| `variant.stats.v1` | r | 13495.000 | 13500.000 | +0.04% | ±11.42% | within-noise |
