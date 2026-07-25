# FASTA 合并

## 用途

将一个或多个 FASTA 文件按顺序合并为单个 FASTA。

## 输入

一个或多个可读取的 FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令先接收输出 FASTA 路径，再接收输入 FASTA 路径。`--allow-duplicate-ids` 会保留重复 ID；默认拒绝重复 ID。`--json` 返回标准结果封装。

## 输出

写入新的合并 FASTA。JSON 返回输入文件数、记录数、残基数、重复 ID 数，以及是否允许重复 ID。

## 示例

```bash
linxira-bio sequence merge merged.fa sample1.fa sample2.fa --json
```

## 结果解读

记录会按输入路径顺序以及各文件内部顺序输出。

## 注意事项

重复 ID 通常会影响下游抽取和索引，因此默认拒绝，除非用户明确允许。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

FASTA 合并遵循常见文本 FASTA 处理行为。

## 故障排除

如果因为重复 ID 合并失败，可以先用 `linxira-bio sequence normalize-ids` 规范化 ID，或者在确认重复有意义时加 `--allow-duplicate-ids`。
