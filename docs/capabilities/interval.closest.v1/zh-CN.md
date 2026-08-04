# 最近基因组区间查询

## 用途

为每个查询 BED 区间查找同一 contig 上一个确定性的最近目标区间。

## 输入

一个查询 BED 和一个目标 BED。程序按魔数识别纯文本和 gzip；接受 BED3 或更宽记录，
但忽略额外列。

## 参数

必须提供 `<query.bed>`、`<target.bed>` 和 `<output.tsv>`；`--json` 返回标准结果封装。

## 输出

带表头 TSV 包含查询 BED3、目标 BED3、非负距离以及 `upstream`、`downstream` 或
`overlap`。JSON 返回匹配、未匹配和各 contig 计数。

## 示例

```bash
linxira-bio interval closest variants.bed genes.bed nearest-genes.tsv --json
```

## 结果解读

坐标采用零起点半开 `[start, end)` 语义。重叠距离为零；首尾相接的区间距离也为零，
但仍保留上游或下游方向。距离相同时选择 `(start, end)` 最小的目标。

## 注意事项

每个查询只返回一个目标。不处理链方向、名称、分数、额外 BED 列、全部并列结果或参考
基因组版本验证。

## 运行时依赖

纯 Rust 本地运行，无需 Python、R、Java、bedtools 或网络。

## 引用

坐标语义遵循 UCSC BED 规范。应报告参考基因组版本以及查询和目标特征来源。

## 故障排除

确认两份文件使用相同参考版本、坐标规则和 contig 命名。精确 contig 上没有目标的查询
会标记为未匹配。
