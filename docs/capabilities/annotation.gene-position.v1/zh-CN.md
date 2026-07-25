# annotation.gene-position.v1

## 用途

把选定的注释特征导出为稳定的坐标与元数据表。

## 输入

一个合法的普通文本或 gzip 压缩 GFF3/GTF 注释。

## 参数

- `--feature-type TYPE`：选择特征类型，可重复；默认为 `gene`。
- `--json`：输出结构化摘要。

## 输出

新的 TSV，包含 ID、名称、序列、起点、终点、链方向、特征类型、父级和来源。

## 示例

```bash
linxira-bio annotation positions input.gff3 genes.tsv --feature-type gene --json
```

## 结果解读

坐标保持 1-based 闭区间；进入下游前检查缺失标识符计数。

## 注意事项

没有 ID、gene_id、transcript_id、locus_tag 或 Name 的匹配记录会被跳过，不会覆盖已有输出。

## 运行时依赖

在 Rust core 中本地运行，不需要外部运行时或网络。

## 引用

未引入外部科学方法；输出是对注释记录的确定性投影。

## 故障排除

如果没有输出行，检查请求的特征类型拼写以及输入中的属性字段。
