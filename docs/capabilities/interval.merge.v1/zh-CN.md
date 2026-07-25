# BED 区间合并

## 用途

在每个 contig 内合并重叠、首尾相接或距离较近的 BED 区间，并写出确定性的 BED3
文件。

## 输入

一个可读取的 BED 文件。程序按魔数识别纯文本和 gzip 流；每条记录至少需要三个制表符
分隔列。

## 参数

必须提供 `<input.bed>` 和 `<output.bed>`。`--max-gap N` 会把间隔不超过 `N` 个碱基的
区间也合并；默认值为 `0`，即只合并重叠和首尾相接的区间。`--json` 返回标准结果封装。

## 输出

写出新的 BED3 文件，包含 `contig`、`start` 和 `end` 三列。JSON 返回输入/输出区间数、
被合并区间数、输入/输出碱基数、`max_gap`、各 contig 统计和警告。

## 示例

```bash
linxira-bio interval merge regions.bed merged.bed --max-gap 10 --json
```

## 结果解读

区间按 `[start, end)` 处理。输出按 contig 和起点排序。`merged_interval_count` 表示被吸收
进更大输出区间、而不是原样输出的输入区间数量。

## 注意事项

v1 只输出 BED3；不会保留 name、score、strand 或额外 BED 列。需要属性处理时，使用
`bedtools merge` 或后续的保留记录版本能力。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java、bedtools 或外部命令行工具。

## 引用

坐标语义遵循 UCSC BED 规范。合并行为与 bedtools 等标准区间代数工具的通用做法一致。

## 故障排除

如果命令拒绝覆盖输出，请选择新的输出路径，或在确认后手动删除旧文件。BED 格式错误中
的行号可用于定位列缺失、坐标非法或区间长度非正的问题。
