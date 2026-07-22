# FASTA 序列统计

## 用途

在本地统计 FASTA 记录数、长度、N50/L50、auN、GC 比例和 N 含量。

## 输入

一个可读取的 FASTA 文件。支持多行序列，标题行必须以 `>` 开头。

## 参数

输入路径为必需位置参数；`--json` 返回标准分析结果封装。

## 输出

返回 `sequence_count`、`total_bases`、最小/最大/平均长度、`n50`、`l50`、
`au_n`、`gc_percent`、`n_count` 和 `n_percent`。

## 示例

```bash
linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json
```

## 结果解读

N50 是达到总长度一半时的序列长度；L50 是达到该阈值所需的序列条数。两者是连续性描述，不代表组装正确性。

## 注意事项

GC 百分比的分母只包含 A/C/G/T；N 百分比使用全部序列字符。统计不会校正污染、倍性或组装错误。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

N50/L50 使用常规定义；auN 为长度加权平均 `sum(length^2) / sum(length)`。

## 故障排除

若提示序列出现在首个标题之前，请检查文件是否确为 FASTA，并移除开头的非标题内容。
