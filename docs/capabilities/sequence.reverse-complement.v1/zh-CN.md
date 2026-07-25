# FASTA 反向互补

## 用途

在本地为 DNA 或 RNA 核苷酸序列生成反向互补 FASTA。

## 输入

一个包含 DNA 或 RNA 核苷酸符号的可读取 FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入和输出 FASTA 路径。`--json` 返回标准结果封装。

## 输出

为每条输入记录写入一条反向互补记录。JSON 返回输入/输出记录数和残基数。

## 示例

```bash
linxira-bio sequence reverse-complement transcripts.fa reverse.fa --json
```

## 结果解读

IUPAC 模糊核苷酸符号会按定义互补。RNA 输入在互补中保留 U；DNA 输入使用 T。

## 注意事项

混用 T 和 U 的记录会被拒绝，以避免静默改变分子类型。不支持蛋白 FASTA。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

互补映射遵循标准 IUPAC 核苷酸模糊符号。

## 故障排除

如果命令拒绝某个符号，请确认输入是核苷酸 FASTA，而不是蛋白 FASTA 或混合 DNA/RNA 导出。
