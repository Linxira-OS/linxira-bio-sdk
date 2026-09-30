# 峰注释

## 用途

用本地 interval.closest 索引为每个 BED 峰分配 GFF3/GTF 注释中最近的基因或特征，产出
确定性的峰-特征对应表，供调控分析使用。

## 输入

一个 BED 峰文件（要求 contig、start、end 三列；第 4 列作为峰名）和一个 GFF3/GTF 注释
文件（明文或 gzip）。

## 参数

`--feature-type` 选择参与比对的特征类型（可重复；默认 `gene`）。输出路径必须不存在。

## 输出

每峰一行的 TSV：峰坐标与名称（1 起始闭区间）、最近特征的 id/名称/类型/区间/链、距离
（重叠为 0）与方向（相对特征链锚点的 `overlapping`/`upstream`/`downstream`）。JSON
汇总峰数、已注释数、未匹配数、按类型的特征计数及警告。

## 示例

```bash
linxira-bio peak annotate peaks.bed genes.gff3.gz annotated.tsv --feature-type gene --json
```

## 结果解读

方向感知链：对 '-' 链特征，位于特征末端之后的峰报告为 `upstream`（朝向 TSS）。未匹配
峰表示该 contig 上没有所请求类型的特征——通常是命名不一致，而非生物学缺失。距离是
基因组间隔，不是调控证据。

## 注意事项

只报告最近的单个特征；等距并列按索引顺序解析。两侧输入的 contig 命名必须完全一致。
少于三列或坐标非法的 BED 行会被拒绝。

## 运行时依赖

除 CLI 外无依赖——扫描是对解析后文本的纯 Rust 实现。

## 引用

报告峰注释时引用 BED 与 GFF3 格式规范（NIC/UCSC；Stein 2013，GFF3）以及产生峰数据的
上游 peak caller。

## 故障排除

全部峰未匹配：对比两侧 contig 命名（`dataset inspect` 可显示两种格式的 contig）。
特征缺 id：注释可能使用非标准属性键；先用 `annotation normalize` 归一化。
