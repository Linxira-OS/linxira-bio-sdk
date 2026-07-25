# FASTA 序列过滤

## 用途

按长度、GC 百分比和 N 百分比在本地过滤 FASTA 记录。

## 输入

一个可读取的 FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入和输出 FASTA 路径。可选过滤参数包括 `--min-length`、`--max-length`、`--min-gc-percent`、`--max-gc-percent` 和 `--max-n-percent`。`--json` 返回标准结果封装。

## 输出

写入包含所有通过过滤记录的新 FASTA。JSON 返回输入/输出记录数和残基数，以及按长度、GC、N 条件拒绝的记录数。

## 示例

```bash
linxira-bio sequence filter contigs.fa kept.fa --min-length 1000 --max-n-percent 5 --json
```

## 结果解读

每条记录独立判断。若记录在较早的过滤条件失败，会计入该首个失败原因。

## 注意事项

GC 百分比使用 canonical A/C/G/T/U 作为分母。过滤不会自动去除污染，也不能验证组装正确性。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

GC 与模糊碱基统计遵循常见 FASTA 序列 QC 定义。

## 故障排除

如果没有记录输出，请放宽阈值，并先运行 `linxira-bio sequence stats INPUT --json` 查看输入概况。
