# 空间转录组学摘要

## 用途

解析 10x Genomics 稀疏表达矩阵（matrix market 格式）及其特征与条形码注释，计算本地质量与汇总统计，包括每个条形码的总计数与检出基因数，以及用于标准“膝点图”评估的条形码排序表。

## 输入

10x 输出目录中的三个文件（支持 gzip 压缩）：矩阵（`matrix.mtx`）、特征注释（`features.tsv` — id、名称、特征类型）与条形码注释（`barcodes.tsv`）。

## 参数

无需参数。

## 输出

TSV 条形码排序表，列为 `rank`、`barcode`、`total_counts`、`n_genes`。JSON 输出额外报告 `format`、`n_barcodes`、`n_features`、`n_nonzero`、`total_counts`、`mean_counts`、`median_genes`、`p90_genes` 与 `barcode_rank`。

## 示例

```bash
linxira-bio medical spatial-transcriptomics matrix.mtx features.tsv barcodes.tsv barcode-rank.tsv --json
```

## 结果解读

每个条形码的 `total_counts` 与 `n_genes` 反映测序深度与检出水平；按总计数排序的排序表复现了用于区分高含量细胞条形码与背景的标准条形码膝点图数据。`median_genes` 与 `p90_genes` 汇总所有条形码的逐条形码检出情况。

## 注意事项

矩阵值按四舍五入的整数计数处理。矩阵维度必须与注释行数完全一致。此为摘要能力：不进行聚类、标准化、细胞分型或空间坐标分析。

## 运行时依赖

无 — 纯本地 Rust 能力（内置 gzip 支持）。

## 引用

10x Genomics 稀疏矩阵格式规范（Matrix Market coordinate）。

## 故障排除

若解析失败，请确认矩阵头声明 `coordinate` 格式、索引为 1 起始，且特征/条形码注释行数等于声明的矩阵维度。gzip 压缩输入会自动识别。
