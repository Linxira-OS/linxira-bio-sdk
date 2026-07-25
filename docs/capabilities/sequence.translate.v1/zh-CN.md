# FASTA 翻译

## 用途

使用 NCBI 标准遗传密码表把核苷酸 FASTA 翻译为蛋白 FASTA。

## 输入

一个可读取的 DNA 或 RNA FASTA 文件。支持纯文本和 gzip 流。

## 参数

命令需要输入和输出 FASTA 路径。可重复使用 `--frame FRAME`，取值为 `-3`、`-2`、`-1`、`1`、`2` 或 `3`；默认 frame 为 `1`。`--trim-terminal-stop` 移除末端 `*`；`--stop-at-first` 在首个终止密码子处停止。`--json` 返回标准结果封装。

## 输出

对每条输入记录和每个请求 frame 写入一条蛋白 FASTA。JSON 返回记录数、残基数、所选 frame 和遗传密码表。

## 示例

```bash
linxira-bio sequence translate cds.fa proteins.fa --frame 1 --trim-terminal-stop --json
```

## 结果解读

输出标题追加 `|frame=+N` 或 `|frame=-N`。模糊密码子翻译为 `X`；终止密码子翻译为 `*`，除非停止相关参数改变输出。

## 注意事项

本版本只实现 NCBI 标准密码表。它不验证 CDS phase、转录本模型、细胞器密码表或生物学完整性。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

密码子翻译使用 NCBI 标准遗传密码表 table 1。

## 故障排除

如果翻译失败，请检查是否含蛋白字符、是否混用 T/U，或所选 frame 是否与目标编码序列一致。
