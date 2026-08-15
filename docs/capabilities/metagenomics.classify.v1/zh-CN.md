# 宏基因组物种分类

## 用途

将读段与 Kraken2 参考数据库比对，生成物种丰度表（clade 与 taxon 读段计数及占比），用于宏基因组群落结构分析。

## 输入

包含读段的 FASTA 或 FASTQ 文件。分类数据库通过 `--database` 指定，必须是 `kraken2-build` 生成或符合标准布局（`hash.k2d`、`opts.k2d`、`taxo.k2d`）的目录。

## 参数

- `--database <目录>`（必填）：Kraken2 数据库目录。
- `--confidence <比例>`：最低置信度阈值（0–1，默认 0.0）。
- `--minimum-hit-groups <n>`：可靠分类所需的最少命中组数（默认 2）。
- `--threads <n>`：工作线程数（默认 1）。

## 输出

TSV 丰度表，列为 `percentage`、`clade_count`、`taxon_count`、`rank`、`taxon_id`、`name`，Kraken2 报告的每个分类节点一行。JSON 输出额外报告 `total_reads`、`classified_reads`、`unclassified_reads`、`classified_fraction` 与 `taxon_count`。

## 示例

```bash
linxira-bio metagenomics classify reads.fq abundance.tsv --database /data/kraken2-db --confidence 0.2 --threads 4 --json
```

## 结果解读

`clade_count` 包含分配至该分类或其任何后代的全部读段；`taxon_count` 仅统计精确分配至该分类的读段。`rank` 采用 Kraken2 编码（`R` 根、`D` 界、`P` 门、`C` 纲、`O` 目、`F` 科、`G` 属、`S` 种、`U` 未分类）。`U` 行报告未分类读段。

## 注意事项

需要 Kraken2 可执行文件及兼容的参考数据库；结果取决于数据库组成与置信度设置。分类为读段级别，不进行基因组组装，也不估算菌株丰度。数据库须符合本地使用许可；受控数据请保持在本地。

## 运行时依赖

Kraken2（2.x）。可通过 Bioconda 安装（`conda install -c bioconda kraken2`）或从 Kraken2 仓库编译。

## 引用

Wood, D.E., Lu, J., & Langmead, B. (2019). Improved metagenomic analysis with Kraken 2. Genome Biology, 20:257.

## 故障排除

若未找到 Kraken2，请通过 Bioconda 或系统包管理器安装。确认 `--database` 目录包含 `hash.k2d`、`opts.k2d` 与 `taxo.k2d`。置信度超出 0–1、线程数超出 1–1024 均会被拒绝。
