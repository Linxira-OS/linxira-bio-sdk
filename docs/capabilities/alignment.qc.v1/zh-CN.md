# SAM 比对质量控制

## 用途

在本地校验 SAM 文本记录，并汇总比对率、FLAG、MAPQ、重复记录和参考序列指标。

## 输入

一个可读的 SAM 文本文件。程序按魔数识别纯文本和 gzip 流；不接受 BAM 或 CRAM。

## 参数

输入路径为必需参数；`--json` 返回标准分析结果封装。v1 不设置科学阈值。

## 输出

返回表头与记录数、primary/secondary/supplementary 记录数、已比对与未比对记录数及
比例、paired/proper-pair/read-1/read-2/duplicate/QC-fail 计数、零 MAPQ 计数、已比对
记录的平均 MAPQ、各参考序列记录数和警告。

## 示例

```bash
linxira-bio alignment qc tests/fixtures/alignment-qc/valid.sam --json
```

## 结果解读

指标按 SAM 比对记录计数，而不是按唯一生物片段计数。同一条 read 可以同时具有
primary、secondary 或 supplementary 记录。MAPQ 的含义和标度取决于比对软件。

## 注意事项

该能力不读取 BAM/CRAM，不校验 CIGAR 与序列长度，不估计插入片段分布，不分析
碱基质量，也不替代 samtools、Picard 或比对软件的完整报告。

## 运行时依赖

纯 Rust 本地流式能力，无需 Python、R、Java、htslib 或外部命令行工具。

## 引用

字段和 FLAG 语义遵循 Global Alliance for Genomics and Health 维护的 SAM/BAM
Format Specification。

## 故障排除

根据错误中的行号定位列、数值字段或 SEQ/QUAL 长度问题。BAM 或 CRAM 请先通过
经过维护的 samtools 工作流转换。
