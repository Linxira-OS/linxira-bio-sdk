# 表格转 FASTA

## 用途

从 CSV 或 TSV 序列表格重建 FASTA 记录。

## 输入

一个带表头的可读取 CSV 或 TSV 表格。支持纯文本和 gzip 流。

## 参数

命令需要输入表格和输出 FASTA 路径。可选参数包括 `--delimiter csv|tsv`、`--id-column`、`--sequence-column`、`--description-column` 和 `--no-description-column`。`--json` 返回标准结果封装。

## 输出

写入新的 FASTA 文件。JSON 返回输入行数、输出记录数、输出残基数、分隔符和使用的列映射。

## 示例

```bash
linxira-bio sequence from-table records.tsv output.fa --delimiter tsv --json
```

## 结果解读

默认情况下，表格需要包含 `id` 和 `sequence` 列。若配置并存在 `description` 列，则会写入 FASTA header 描述部分。

## 注意事项

ID 不能为空且不能包含空白字符。sequence 单元格内部的空白会在写入 FASTA 前移除。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

转换逻辑遵循常见 FASTA header 和序列行构造方式。

## 故障排除

如果因缺少列转换失败，请传入 `--id-column`、`--sequence-column` 或 `--description-column` 匹配实际表头。
