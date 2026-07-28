# 短读段参考比对

## 用途

使用固定的 `minimap2 -x sr` 参数将一个本地短读段 FASTQ 比对至本地 FASTA 参考序列，再由 samtools 生成坐标排序 BAM。

## 输入

输入一个参考 FASTA 和一个单端 FASTQ 文件。双端感知比对不属于此 v1 契约。

## 参数

设置范围为 1 到 1024 的 `--threads N`。

## 输出

生成一个新的坐标排序 BAM 和 JSON 执行元数据；流程不会覆盖输入或已有输出。

```bash
linxira-bio alignment short-read reference.fa reads.fastq aligned.bam --threads 4 --json
```

## 示例

上述命令生成的 BAM 可继续用于本地质量或覆盖度报告。

## 结果解读

得出生物学结论前，应检查比对质量、比对率和覆盖度。

## 注意事项

应针对实验类型选择合适的比对器与预设。此 v1 流程仅支持单端，不替代专用双端或变异检测流程。

## 运行时依赖

需要 `PATH` 中的 `minimap2` 和 `samtools`，或设置 `LINXIRA_BIO_MINIMAP2` 与 `LINXIRA_BIO_SAMTOOLS`；两个进程均不经过 shell 调用。

## 引用

引用 minimap2、samtools、其版本、参考基因组版本和读段预处理方法。

## 故障排除

审计 `minimap2` 与 `samtools`，确保本地磁盘空间充足，并检查 FASTQ 与 FASTA 是否可读。
