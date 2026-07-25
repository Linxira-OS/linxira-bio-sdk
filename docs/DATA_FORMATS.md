# Data Format Matrix

This document separates four different claims: content recognition, bounded
preview, executable biological analysis, and result export. Recognition never
means that an analysis capability is available.

## Import And Analysis

| Format | Content detection | Bounded preview | Available analysis | Current boundary |
| --- | --- | --- | --- | --- |
| FASTA | Yes | Sequence records | `sequence.stats.v1` | Plain, gzip, and BGZF |
| FASTQ | Yes | Read records | `fastq.qc.v1`, `fastq.trim.v1`, `fastq.adapter.v1` | QC plus FASTQ output for 3' quality trimming and adapter removal; plain, gzip, and BGZF input |
| CSV | Yes | Parsed table | `expression.matrix.qc.v1`, `table.manipulate.v1` | Rectangular expression matrices; quoted and multiline preview fields supported; row/column manipulation supported |
| TSV | Yes | Parsed table | `expression.matrix.qc.v1`, `table.manipulate.v1` | Rectangular expression matrices; row/column manipulation supported |
| BED | Yes | Interval rows | `interval.intersect.v1`, `interval.merge.v1`, `interval.subtract.v1` | Pairwise half-open overlap summary plus BED3 merge/subtract outputs |
| GFF3 | Yes | Feature rows | None | Inspection only |
| GTF | Yes | Feature rows | None | Inspection only |
| VCF | Yes | Variant rows | `variant.stats.v1` | Plain, gzip, and BGZF; no BCF |
| SAM | Yes | Alignment rows | `alignment.qc.v1` | Text SAM flag and mapping QC; plain or gzip |
| BAM | Magic bytes only | Binary metadata | None | `recognized-unsupported` |
| BCF, CRAM | Magic bytes only | Binary metadata | None | `recognized-unsupported` |
| HDF5, H5AD, LOOM | Signature plus extension hints | Binary metadata | None | Domain import is planned |
| RDS | Magic bytes only | Binary metadata | None | `recognized-unsupported` |
| PDB | Recognized text structure | 3D GUI and PNG snapshot | `structure.pdb.summary.v1` | Plain or gzip; coordinates and optional explicit pLDDT; GUI infers display bonds only |
| mmCIF | Recognized text structure | 3D GUI and PNG snapshot | None | Plain or gzip GUI coordinate viewing; analysis remains planned |
| ZIP | Container signature | Archive metadata | None | Never extracted by inspection |

Content takes precedence over a misleading filename extension. A supported
preview is capped at 200 records or 10 MiB of uncompressed payload and is not
proof that the remainder of a file is valid. Binary files report truncation
against their actual payload size.

## Result Export

| Format | Intended use | Rules |
| --- | --- | --- |
| CSV | Default interchange format | Stable columns and RFC 4180-compatible quoting |
| TSV | Bioinformatics command-line interoperability | Stable columns with tab delimiters |
| JSON | Complete structured result | Preserves the input JSON value |
| JSONL | Record streams and agents | Requires one object or an array of objects |
| XLSX | Spreadsheet users | Large integers that cannot be represented exactly are written as text |

CSV is the default recommendation for portable tables. Keep biological domain
files such as VCF, BED, and GFF3 in their native format when round-trip domain
semantics matter. XLSX output is limited to 1,048,576 rows and 16,384 columns,
including the header.

## 中文说明

“识别”“预览”“可执行分析”和“导出”是四种不同承诺。文件被识别并不代表已有可运行
的生物学分析能力。FASTA、FASTQ、SAM 和 VCF 当前分别可运行序列统计、读段质量
控制/质量裁剪/接头去除、文本比对质控和变异描述统计；BED 可计算两组区间的半开重叠摘要，CSV/TSV
可进行矩形表达矩阵质控，PDB 可生成结构摘要和供渲染使用的原子坐标。PDB/mmCIF
均可在 GUI 中进行有界 3D 预览并导出当前视角 PNG，但 mmCIF 分析仍未实现。GFF3、GTF 目前只做有界
预览；BAM、BCF、CRAM、H5AD 等二进制格式仅识别，不会伪装成可用能力。

预览最多读取 200 条记录或 10 MiB 解压后内容。表格默认导出 CSV，也支持 TSV、
JSON、逐行对象 JSONL 和 XLSX。需要保留 VCF、BED、GFF3 等领域语义时，应保留
原始领域格式，不应把表格导出当作无损往返转换。
