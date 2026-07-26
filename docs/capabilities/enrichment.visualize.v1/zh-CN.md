# 富集结果可视化

## 用途

把通用、GO 或 KEGG 过度富集结果在本地绘制为柱状图、气泡图或术语—基因网络 SVG。

## 输入

查询标识列表和 CSV/TSV 术语关联表。

## 参数

选择 `custom`、`go` 或 `kegg`，选择 `bar`、`dot` 或 `network`，并设置最小重叠数和最多术语数。

## 输出

SVG 文件，以及描述图形类型、尺寸、轨道数、图元数、路径和警告的 JSON 元数据。

## 示例

```bash
linxira-bio enrichment visualize genes.txt associations.tsv enrichment.svg --kind go --style dot --json
```

## 结果解读

柱状图和气泡图按校正显著性展示条目；网络图连接报告术语与重叠查询基因。

## 注意事项

结果依赖输入背景和关联表；可视化不会额外执行本体传播或语义去冗余。

## 运行时依赖

统计和 SVG 绘制均由本地 Rust 完成，无需网络。

## 引用

记录关联库版本、查询背景、富集与校正方法以及本能力版本。

## 故障排除

确认查询标识存在于关联背景中，并让 `--kind` 与术语标识类型一致。
