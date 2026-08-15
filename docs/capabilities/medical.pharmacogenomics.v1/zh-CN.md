# 药物基因组学变异解读

## 用途

针对 VCF 中的常见药物基因组学（PGx）星等位基因，使用内置的离线等位基因表（GRCh38）进行解读，并生成包含等位基因后果、表型与受影响药物的表格。

## 输入

包含变异记录的 VCF 文件（支持 gzip 压缩）。内置表按变异的染色体、位置、参考碱基与替代碱基进行匹配。

## 参数

无需参数。

## 输出

TSV 解读表，列为 `chrom`、`position`、`reference`、`alternate`、`rsid`、`gene`、`allele`、`consequence`、`phenotype`、`drugs`、`genotype`（当 VCF 含样本基因型时为 hom-alt/het-alt/ref）。JSON 输出额外报告 `reference_build`、`record_count`、`matched_variant_count`、`allele_count`、`genes_affected`、`variants` 与 `combined_phenotypes`。

## 示例

```bash
linxira-bio medical pharmacogenomics variants.vcf interpretation.tsv --json
```

## 结果解读

纯合参考基因型的记录不计入匹配（要求存在等位基因）。当 CYP2C19 或 CYP2D6 存在多个等位基因时，报告合并双倍型表型（例如 `CYP2C19*2/*3` → 慢代谢者）。其他基因仅报告等位基因级后果。

## 注意事项

仅供研究使用：不构成临床解读或用药建议。内置表仅覆盖少量常见的 GRCh38 星等位基因与标签变异；未匹配不表示代谢正常。基因型直接从 VCF 样本列读取，不做二次验证。坐标为 GRCh38；其他版本的变异需先进行 liftover。

## 运行时依赖

无 — 为纯本地 Rust 能力，使用离线等位基因表。

## 引用

参考事实整理自公开药物基因组学文献与数据库记录；任何临床使用前请与原始基因型结果核对。

## 故障排除

若没有等位基因匹配，请确认 VCF 使用 GRCh38 坐标，且样本基因型列包含替代等位基因。确认 REF/ALT 字符串与表完全一致（链与等位基因大小写敏感）。
