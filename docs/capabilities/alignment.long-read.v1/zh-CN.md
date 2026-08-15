# 长读长比对

## 用途

使用 minimap2 将长读长测序 reads（ONT 或 PacBio）比对到参考基因组，支持可配置的预设。

## 输入

FASTA 格式的参考基因组和 FASTQ 格式的长读长 reads。

## 参数

`--preset` 选择比对预设：`map-ont`、`map-pb` 或 `map-hifi`（默认 `map-ont`）。
`--threads` 设置线程数（默认 1）。

## 输出

包含比对 reads 的 SAM 文件。JSON 结果包裹原生工具执行元数据，包括已比对和未比对 reads 数量。

## 示例

```bash
linxira-bio alignment long-read reference.fa reads.fastq output.sam --preset map-ont --threads 4 --json
```

## 结果解读

审查比对率和比对质量分布。低比对率可能表示预设不匹配、reads 质量低或参考基因组距离较远。

## 注意事项

需要安装 minimap2。预设应与测序技术匹配。SAM 输出可能很大，建议通过管道传给 samtools 转换为 BAM。

## 运行时依赖

minimap2 可执行文件。设置 `LINXIRA_BIO_MINIMAP2` 可覆盖二进制路径。

## 引用

Li H. Minimap2: pairwise alignment for nucleotide sequences. Bioinformatics.
2018;34(18):3094-3100.

## 故障排除

若找不到 minimap2，请安装或将 `LINXIRA_BIO_MINIMAP2` 设置为正确路径。验证预设与测序平台匹配。
已有输出文件不会被覆盖。