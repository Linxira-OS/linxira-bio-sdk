# 序列格式转换（Biopython）

## 用途

使用锁定版本的 Biopython 工作流包，在 FASTA、FASTQ、GenBank 与 EMBL 之间转换
生物序列文件。转换是严格的：以声明的输入格式解析记录，以声明的输出格式写出，
不做任何静默重解释。

## 输入

一个 FASTA、FASTQ、GenBank 或 EMBL 格式的序列文件（未压缩）。

## 参数

- `--input-format fasta|fastq|genbank|embl` — 覆盖按扩展名推断的输入格式。
- `--output-format fasta|fastq|genbank|embl` — 覆盖按扩展名推断的输出格式。

## 输出

目标路径下的转换后文件，以及 JSON 结果信封，包含 `records_written`、
`input_format`、`output_format`、转换产物（路径、大小、SHA-256）和
provenance（CPython、Biopython、NumPy 版本与依赖锁哈希）。

## 示例

```bash
linxira-bio sequence convert input.fasta output.genbank --output-format genbank
linxira-bio sequence convert reads.fastq reads.fa --output-format fasta
```

## 结果解读

核对输出记录数与输入一致。FASTA 转 FASTQ 不受支持，因为 FASTA 记录没有质量
分数；pack 会以错误信封拒绝该请求。FASTA 转 GenBank 或 EMBL 受支持：FASTA
无法携带 molecule_type，pack 会为缺失该字段的记录写入默认值 `DNA`；GenBank
或 EMBL 输入声明的 molecule_type 会保留。

## 注意事项

该能力执行 `org.linxira.sequence-conversion-biopython` 工作流包，需要
CPython 3.12 解释器以及 pack 哈希锁中固定的 Biopython 与 NumPy 版本。
已有输出文件不会被覆盖。该能力不支持压缩输入。

## 运行时依赖

Python 3.12.x 与 `biopython==1.85`、`numpy==2.2.4`，通过 pack 锁解析
（`workflows/org.linxira.sequence-conversion-biopython/requirements.lock`）。

## 引用

Cock PJ et al. Biopython: freely available Python tools for computational
molecular biology and bioinformatics. Bioinformatics. 2009;25(11):1422-1423.
<https://doi.org/10.1093/bioinformatics/btp163>

## 故障排除

- 提示 "cannot infer a sequence format from extension" — 显式传入
  `--input-format` 或 `--output-format`。
- 非零退出且信封为 `status: error` — pack 拒绝了请求，查看诊断消息。
- 输出路径不得已存在。
