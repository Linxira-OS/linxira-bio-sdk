# FASTQ 读段质量控制

## 用途

以流式方式计算 FASTQ 的总体及逐循环质量指标，无需把完整数据集载入内存。

## 输入

一个可读取的 FASTQ 文件。程序按内容识别纯文本、gzip 和 BGZF；支持换行折叠的
序列与质量行，每条记录的序列长度和质量字符长度必须一致。

## 参数

输入路径为必需参数。`--quality-encoding` 可取 `auto`（默认）、`phred+33` 或
`phred+64`。`--max-cycles` 默认只输出前 500 个循环的逐循环指标，不限制总体
统计。`--json` 返回标准分析结果封装。

## 输出

返回读段数、碱基数、最小/最大/平均读长、GC 和 N 比例、平均质量、Q20/Q30
比例、检测与实际采用的质量编码、逐循环指标及警告。

## 示例

```bash
linxira-bio fastq qc tests/fixtures/fastq-qc/valid.fastq --quality-encoding phred+33 --json
```

## 结果解读

质量指标使用 `applied_quality_offset`。自动模式下，若字符同时兼容历史上的
Phred+33 与 Phred+64/Solexa 范围，结果会标记 `quality_encoding: ambiguous`、
给出警告并按 Phred+33 计算。只有仪器或上游流程元数据能够确认编码时才应覆盖。
逐循环上限警告不影响总体指标。

## 注意事项

当前版本不检测接头、重复、过度富集序列、污染或仪器特异性问题。仅凭 Q20/Q30
比例不能判断读段是否适合某项下游生物学分析。

## 运行时依赖

纯 Rust 本地流式能力，无需 Python、R、Java 或外部生物信息学工具。

## 引用

FASTQ 质量编码历史参考 Cock 等，2010，Nucleic Acids Research
38(6):1767-1771，doi:10.1093/nar/gkp1137。

## 故障排除

记录损坏或截断时，按错误中的记录号和行号检查源文件。自动编码含糊时，应查询
测序仪或上游流程元数据，不要根据高质量字符直接猜测。
