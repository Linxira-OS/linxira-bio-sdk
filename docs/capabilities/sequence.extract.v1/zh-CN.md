# FASTA 序列提取

## 用途

按 FASTA 标识符提取完整记录，也可按 1-based inclusive 坐标从本地 FASTA 中提取片段。

## 输入

一个可读取的 FASTA 文件。支持纯文本和 gzip 流。标题行必须包含非空的第一个标识符字段。

## 参数

命令需要输入路径和输出 FASTA 路径。使用 `--id ID` 提取完整记录，使用 `--region ID:START-END[:+|-]` 提取坐标区间。`--strict` 会在任何标识符或区域目标缺失时失败。`--json` 返回标准结果封装。

## 输出

写入新的 FASTA 文件，并拒绝覆盖既有输出。JSON 返回输入/输出记录数、残基数、请求与命中的标识符、请求与输出的区域数以及缺失 selector。

## 示例

```bash
linxira-bio sequence extract genome.fa selected.fa --id chr1 --region chr2:100-250:- --strict --json
```

## 结果解读

完整记录输出保留原始 FASTA 标题。区域输出使用 `ID:START-END:+` 或 `ID:START-END:-` 标题；负链区域会对提取片段做反向互补。

## 注意事项

坐标为 1-based 且闭区间。本能力不解释 BED、GFF、GTF、CDS phase、外显子、内含子或转录本模型；注释引导提取属于独立能力。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

FASTA 解析和反向互补行为遵循常用 IUPAC 核苷酸表示法。

## 故障排除

如果提示 selector 缺失，请确认请求标识符与 `>` 后第一个空白分隔字段一致，并确认区域坐标未超过记录长度。
