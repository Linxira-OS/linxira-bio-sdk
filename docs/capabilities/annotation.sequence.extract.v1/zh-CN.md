# annotation.sequence.extract.v1

## 用途

从匹配的参考 FASTA 提取基因、转录本、CDS、外显子、UTR 或启动子。

## 输入

- 一个合法的 GFF3/GTF 注释。
- 一个标题标识符与注释序列 ID 匹配的 FASTA。
- 支持普通文本与 gzip 压缩输入。

## 参数

- `--feature-type`：`gene`、`transcript`、`cds`、`exon`、`utr`、`five_prime_utr`、`three_prime_utr` 或 `promoter`；默认为 `gene`。
- `--promoter-length N`：正整数启动子长度；默认为 `1000`。
- `--json`：输出结构化摘要。

## 输出

新的 FASTA，以及匹配、输出、缺失参考、跳过特征和碱基数统计。

## 示例

```bash
linxira-bio annotation extract genes.gff3 genome.fa cds.fa --feature-type cds --json
```

## 结果解读

多片段会拼接，负链结果会反向互补，CDS 会逐片段应用 phase。

## 注意事项

该能力不会下载参考或推断缺失父级。启动子由基因坐标推导并按参考序列边界裁剪，不会覆盖已有输出。

## 运行时依赖

在 Rust core 中本地运行，不需要 Python、R、Java、GPU、容器或网络。

## 引用

未使用外部预测方法；提取遵循标准注释坐标和链方向。

## 故障排除

如果输出缺失，比较注释序列 ID 与 FASTA 标题第一个字段，并检查缺失参考和跳过特征计数。
