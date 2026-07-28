# BAM 和 CRAM 质量报告

## 用途

使用维护中的 `samtools stats` 从本地 BAM 或 CRAM 文件生成比对质量报告。

## 输入

输入一个本地 BAM 或 CRAM 比对文件。若 CRAM 未包含解码所需参考碱基，提供本地 FASTA 参考序列。

## 参数

仅在原生解码器需要时使用 `--reference reference.fasta`。

## 输出

生成完整的制表符分隔原生报告和 JSON 执行元数据，拒绝覆盖已有输出。

```bash
linxira-bio alignment bam-cram-qc sample.bam alignment-stats.tsv --json
```

## 示例

运行上述命令保留全部比对统计信息，供后续检查或表格处理。

## 结果解读

结合建库和比对方法检查报告中的比对读段数、重复率、插入片段长度和质量区段。

## 注意事项

该能力用于科研分析，不用于临床诊断。CRAM 解码可能需要编码时的精确参考序列。工件 Worker 仅接受自包含比对输入；需要外部参考序列时使用 CLI。

## 运行时依赖

需要 `PATH` 中的 `samtools`，或设置 `LINXIRA_BIO_SAMTOOLS`；执行过程不经过 shell。

## 引用

引用 samtools、其版本、参考基因组版本和生成输入的比对器。

## 故障排除

审计 `samtools` 环境；CRAM 解码失败时检查参考序列与 contig 名称。
