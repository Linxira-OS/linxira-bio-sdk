# 变异检测

## 用途

从按坐标排序且已建索引的 BAM/CRAM 比对结果出发、对照参考 FASTA 检测小变异（SNV 与
indel），补齐 FASTQ 到 VCF 的流水线断链。

## 输入

已建索引、按坐标排序的比对文件（BAM 或 CRAM）与已建索引的参考 FASTA。比对能力产出的
短读比对结果已满足要求。

## 参数

`--min-mq`（默认 20）按比对质量过滤 pileup 读段；`--min-bq`（默认 13）按质量过滤碱基；
两者取值 0-255。`--threads` 设置 bcftools 工作线程。`--all-sites` 输出无替代等位基因的
参考位点；默认只输出变异位点。输入与输出路径必须不同。

## 输出

在指定路径写出 VCF。JSON 报告工具 `bcftools`、模式 `call`、输出路径与字节数、线程数、
`command_count` 为 2、以及警告。中间 BCF 暂存于输出旁的临时目录并随后删除。

## 示例

```bash
linxira-bio variant call sample.sorted.bam reference.fa calls.vcf --min-mq 20 --min-bq 13 --json
```

## 结果解读

始终使用多等位基因调用模型。默认阈值遵循 bcftools 调用惯例；调整灵敏度时建议收紧而非
放宽。基因型质量与队列级过滤属于变异过滤能力，不属于本调用。

## 注意事项

需要 PATH 上的 `bcftools` 或经 `LINXIRA_BIO_BCFTOOLS` 指定；该二进制仅被探测与调用，
绝不捆绑分发。队列级联合分型与结构变异检测不在范围内。

## 运行时依赖

原生 `bcftools` 可执行文件（环境审计在 variant-files 类别下探测）。不使用 Python、R 或
Java 运行时。

## 引用

报告检测结果时引用 Li 等 2009（Bioinformatics 25:2078-2079）的 SAMTools/bcftools
pileup 模型，及其手册中记载的 bcftools call 多等位基因模型。

## 故障排除

先运行环境审计。确认比对文件按坐标排序且索引在侧、参考序列带 `.fai` 索引；未排序的
输入会被 bcftools 以排序错误拒绝。
