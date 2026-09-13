# expression.quantify.v1

对单端或双端 reads 用 `salmon quant`（预建索引）做转录本定量并汇总结果表。
引擎以受控、无 shell 的参数编排原生工具；完整 `quant.sf` 表原样保留为产物，
并归约出机器可读摘要，用于与传统管线的 quant.sf 逐字段对照（行数、总量）。

## 用途

用 salmon 对 reads 定量，同时保留原始定量表和可比对的摘要，使 Rust 编排运行
与既有传统管线输出可以逐字段核验。

## 输入

- 一个单端 reads 文件（`fasta`/`fastq`，可 gzip）或恰好两个双端 mate 文件。
- 预建好的 salmon 索引目录（`--index`）；本能力不负责建索引。

## 参数

- `--index DIR`（必填）：salmon 索引目录。
- `--lib-type A`（默认 `A` 自动检测）：salmon 文库类型。
- `--threads N`（默认 1）：salmon 并行度。
- `--no-validate-mappings`：关闭 `--validateMappings`（默认启用选择性比对模式）。
- `--json`：输出标准结果信封。
- salmon 可执行文件经 `LINXIRA_BIO_SALMON` 或 `PATH` 解析。

## 输出

- `--output quant.sf`（默认 `quant.sf`）：原样 quant.sf 表
  （Name、Length、EffectiveLength、TPM、NumReads）。
- 摘要字段：tool、lib_type、thread_count、transcript_count、
  expressed_transcript_count、total_tpm、total_num_reads、top_transcripts
  （TPM 前 5 及 NumReads 占比）、output_bytes、warnings。

## 示例

```bash
linxira-bio expression quantify r1.fq.gz r2.fq.gz --index salmon_idx --threads 8 --output run1/quant.sf --json
```

## 结果解读

- `transcript_count` 必须等于索引中的转录本数；与传统管线 quant.sf 的行数
  对比是第一道交叉校验。
- `total_num_reads` 是估计的已分配 reads；与传统运行的差异反映 salmon 参数
  （文库类型、validateMappings），参数已在摘要中披露。

## 注意事项

- 需要本地安装 salmon；引擎绝不建索引、不下载参考。
- gzip FASTQ 直接透传给 salmon。

## 运行时依赖

- salmon（近期 1.x 均可）；用 `LINXIRA_BIO_SALMON` 或 PATH 解析。
- 不涉及 Python/R 运行时。

## 引用

- Patro, R. et al. (2017). Salmon provides fast and bias-aware quantification
  of transcript expression. Nature Methods, 14, 417–419.

## 故障排除

- 找不到 `salmon`：安装 salmon 或设置 `LINXIRA_BIO_SALMON`。
- 与传统运行 quant.sf 行数不一致：先核对索引构建与 salmon 版本，再排查编排。
