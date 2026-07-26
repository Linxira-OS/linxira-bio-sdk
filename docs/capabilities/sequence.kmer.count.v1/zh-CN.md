# 精确 k-mer 计数

## 用途

使用 Rust 压缩键在本机精确统计 FASTA k-mer，并可合并反向互补序列。

## 输入

一个纯文本、gzip 或 BGZF FASTA 文件。

## 参数

`--k` 范围为 1 到 31；`--canonical` 合并反向互补；`--top-n` 控制 JSON 预览数量。

## 输出

输出含 `kmer`、`count` 的完整 TSV，以及窗口总数、不同 k-mer 数、歧义窗口数和高频预览。

## 示例

```bash
linxira-bio sequence kmer-count input.fa kmers.tsv --k 21 --canonical --top-n 50 --json
```

## 结果解读

`counted_windows` 仅包含 A/C/G/T/U 窗口；U 规范为 T，歧义窗口单独报告。

## 注意事项

这是精确计数，不是基因组大小估计、测序错误模型或近似 sketch。

## 运行时依赖

纯 Rust 本地运行，无需 Python、R、Java 或外部程序。

## 引用

canonical 计数选取 k-mer 与其反向互补压缩编码中的较小值。

## 故障排除

确认 `k` 不超过 31，且输入为有效 FASTA；已有输出文件不会被覆盖。
