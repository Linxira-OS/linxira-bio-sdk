# FASTA 转表格

## 用途

将 FASTA 记录转换为 CSV 或 TSV 行，便于表格查看、连接分析和 agent 可读的表格工作流。

## 输入

一个可读取的 FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入 FASTA 和输出 CSV/TSV 路径。可选参数包括 `--delimiter csv|tsv` 和 `--no-header`。`--json` 返回标准结果封装。

## 输出

写入包含 `id`、`description`、`length`、`sequence` 四列的 CSV 或 TSV。JSON 返回行数、残基数、分隔符、是否写入表头和列名。

## 示例

```bash
linxira-bio sequence to-table input.fa records.tsv --delimiter tsv --json
```

## 结果解读

每条 FASTA 记录对应一行。description 是 header 中第一个空白分隔 ID 后面的文本。

## 注意事项

sequence 列面向文本序列数据。非 UTF-8 序列字节会被拒绝，而不会静默改写。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

表格结构遵循常见 FASTA ID、描述、长度和序列字段。

## 故障排除

如果下游表格读取器识别错分隔符，请显式传入 `--delimiter csv` 或 `--delimiter tsv`。
