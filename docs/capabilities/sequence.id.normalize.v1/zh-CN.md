# FASTA 标识符规范化

## 用途

按确定性规则重写 FASTA 记录 ID，方便后续工具使用短 ID、稳定 ID 或统一编号 ID。

## 输入

一个可读取的 FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入和输出 FASTA 路径。可选参数包括 `--prefix`、`--start`、`--width`、`--no-padding` 和 `--drop-description`。`--json` 返回标准结果封装。

## 输出

写入 ID 已重写的新 FASTA。JSON 返回输入/输出记录数、残基数、前缀、首末编号、宽度，以及是否保留描述信息。

## 示例

```bash
linxira-bio sequence normalize-ids input.fa renamed.fa --prefix seq --width 6 --json
```

## 结果解读

ID 按输入顺序分配。若前缀为 `seq`、起始值为 `1`、宽度为 `6`，第一条记录会变成 `seq000001`。

## 注意事项

该能力不会推断生物学基因名，也不会自动生成外部映射表。需要可逆审计时请保留原始 FASTA。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

ID 规范化遵循常见 FASTA 记录命名实践。

## 故障排除

如果下游工具仍拒绝文件，请使用不含空白和特殊标点的前缀，并手动检查输出 header。
