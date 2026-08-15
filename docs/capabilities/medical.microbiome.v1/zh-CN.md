# 微生物组多样性分析

## 用途

使用 Kraken2 对宏基因组读段进行分类（与 `metagenomics.classify.v1` 共享），并计算 α 多样性汇总（物种丰富度、Shannon 指数、Pielou 均匀度）及优势物种级分类单元，供研究用途的微生物组分析使用。

## 输入

包含读段的 FASTA 或 FASTQ 文件，以及 Kraken2 数据库目录（`--database`）。

## 参数

- `--database <目录>`（必填）：Kraken2 数据库目录。
- `--confidence <比例>`：最低置信度阈值（0–1，默认 0.0）。
- `--minimum-hit-groups <n>`：可靠分类所需的最少命中组数（默认 2）。
- `--threads <n>`：工作线程数（默认 1）。

## 输出

TSV 分类丰度表（布局与 `metagenomics.classify.v1` 相同）。JSON 输出报告分类总量以及 `species_richness`、`shannon_index`、`evenness` 与 `top_species`（按读段数前 5 的物种及其占比）。

## 示例

```bash
linxira-bio medical microbiome reads.fq abundance.tsv --database /data/kraken2-db --confidence 0.2 --threads 4 --json
```

## 结果解读

Shannon 指数基于物种级（`S`）分类单元的读段计数计算；均匀度为 Shannon 除以物种丰富度的自然对数。丰富度为 0 或 1 时均匀度为 0。`classified_fraction` 与 `unclassified_reads` 汇总总体分类成功率。比较样本时应保持测序深度一致或使用稀有化比例。

## 注意事项

需要本地安装 Kraken2 可执行文件及兼容数据库。仅供研究使用：读段级分类不进行基因组组装、不估算菌株丰度，也不能替代临床微生物组诊断。多样性结果取决于数据库组成与置信度设置。

## 运行时依赖

Kraken2（2.x），可通过 Bioconda 安装（`conda install -c bioconda kraken2`）。

## 引用

Wood, D.E., Lu, J., & Langmead, B. (2019). Improved metagenomic analysis with Kraken 2. Genome Biology, 20:257.

## 故障排除

若未检出物种，请检查数据库内容与置信度设置，并确认读段适合所用数据库（核苷酸数据库对应鸟枪法读段）。置信度超出 0–1、线程数超出 1–1024 均会被拒绝。
