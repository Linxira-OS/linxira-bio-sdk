# 比对覆盖度汇总

## 用途

使用 `samtools coverage` 为 BAM 或 CRAM 比对生成本地深度和广度汇总。

## 输入

输入一个本地 BAM 或 CRAM 文件。

## 参数

当 CRAM 需要外部参考碱基时，在 CLI 中使用 `--reference reference.fasta`。

## 输出

生成制表符分隔覆盖度表和 JSON 执行元数据，不会覆盖已有输出。

```bash
linxira-bio alignment coverage sample.bam coverage.tsv --json
```

## 示例

使用上述命令生成每个参考序列的覆盖广度和深度数值。

## 结果解读

仅在确认过滤和比对参数可比较后，再比较不同参考序列、样本和文库的覆盖广度及平均深度。

## 注意事项

覆盖度受重复、过滤、参考序列组成和输入排序影响。本能力报告原生结果，不进行诊断。

## 运行时依赖

需要 `PATH` 中的 `samtools` 或 `LINXIRA_BIO_SAMTOOLS`；调用不经过 shell。

## 引用

引用 samtools、其版本、参考基因组版本以及上游比对和过滤方法。

## 故障排除

确认 samtools 可读取 BAM/CRAM；CRAM 解码时提供正确参考序列。
