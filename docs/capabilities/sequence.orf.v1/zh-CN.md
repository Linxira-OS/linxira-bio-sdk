# FASTA ORF 预测

## 用途

在本地从核苷酸 FASTA 记录中查找完整 ORF 和可选的 3' 端 partial ORF。

## 输入

一个可读取的 DNA 或 RNA FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入和输出 FASTA 路径。`--min-amino-acids N` 设置最小蛋白长度。使用 `--forward-only` 跳过反向链搜索；使用 `--include-partial-3prime` 输出从 ATG 开始但到序列末端仍无终止密码子的 ORF。`--json` 返回标准结果封装。

## 输出

以 FASTA 写出预测 ORF 的蛋白序列。JSON 返回输入/输出记录数和残基数、含 ORF 的记录数、完整与 partial ORF 数、最长 ORF 长度、最小长度以及是否启用反向链搜索。

## 示例

```bash
linxira-bio sequence orf contigs.fa orfs.faa --min-amino-acids 30 --include-partial-3prime --json
```

## 结果解读

输出标题包含序号、链、frame、1-based 起点、闭区间终点，以及 complete 或 partial 状态。完整 ORF 的蛋白序列不包含末端终止密码子。

## 注意事项

这是确定性的 ORF 查找器，不是基因预测软件。它不建模内含子、密码子偏好、非标准遗传密码表、替代起始密码子或注释证据。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

ORF 使用 NCBI 标准遗传密码表下的 ATG 起始和 TAA/TAG/TGA 终止来识别。

## 故障排除

如果 ORF 数过少，请降低 `--min-amino-acids`、启用 partial ORF，并确认序列方向。
