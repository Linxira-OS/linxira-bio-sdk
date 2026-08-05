# annotation.gxf.to-bed.v1

## 用途

将选定的注释特征从 GFF3 或 GTF 格式转换为 BED6 格式。

## 输入

一个合法的普通文本或 gzip 压缩 GFF3/GTF 注释。

## 参数

- `--feature-types LIST`：逗号分隔的要转换的特征类型；每个类型匹配不区分大小写，默认为 `gene`。
- `--json`：输出结构化摘要。

## 输出

新的 BED6 文件，包含 chrom、start（0-based）、end、name、score 和 strand 列。name 字段取自 ID、Name、gene_id、transcript_id 或 locus_tag 中第一个可用的属性。

## 示例

```bash
linxira-bio annotation to-bed input.gff3 output.bed --feature-types gene,exon --json
```

## 结果解读

BED 坐标为 0-based 半开区间。进入下游前检查跳过的无标识符记录计数。

## 注意事项

没有可用标识符的匹配记录会被跳过，不会覆盖已有输出。无法解析为浮点数的 score 值默认为 0。

## 运行时依赖

在 Rust core 中本地运行，不需要外部运行时或网络。

## 引用

未引入外部科学方法；输出是对注释记录的确定性投影。

## 故障排除

如果没有输出行，检查请求的特征类型拼写以及输入中的属性字段。