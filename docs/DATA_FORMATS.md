# Data Format Matrix

This document separates four different claims: content recognition, bounded
preview, executable biological analysis, and result export. Recognition never
means that an analysis capability is available.

## Import And Analysis

| Format | Content detection | Bounded preview | Available analysis | Current boundary |
| --- | --- | --- | --- | --- |
| FASTA | Yes | Sequence records | `sequence.stats.v1`, `sequence.kmer.count.v1`, `primer.epcr.v1`, `variant.normalize.v1` reference, `protein.properties.v1` | Plain, gzip, and BGZF; protein properties reject gap, stop, digit, and unsupported symbols |
| FASTQ | Yes | Read records | `fastq.qc.v1`, `fastq.trim.v1`, `fastq.adapter.v1` | QC plus FASTQ output for 3' quality trimming and adapter removal; plain, gzip, and BGZF input |
| CSV | Yes | Parsed table | `expression.matrix.qc.v1`, `expression.normalize.v1`, `expression.pca.v1`, `expression.cluster.v1`, `expression.heatmap.v1`, `table.manipulate.v1`, `set.venn.v1`, `set.upset.v1` | Rectangular expression analysis, general table manipulation, and named-column exact set-overlap analysis |
| TSV | Yes | Parsed table | `expression.matrix.qc.v1`, `expression.normalize.v1`, `expression.pca.v1`, `expression.cluster.v1`, `expression.heatmap.v1`, `table.manipulate.v1`, `set.venn.v1`, `set.upset.v1` | Rectangular expression analysis, row/column manipulation, and named-column exact set-overlap analysis |
| BED | Yes | Interval rows | `interval.intersect.v1`, `interval.merge.v1`, `interval.subtract.v1` | Pairwise half-open overlap summary plus BED3 merge/subtract outputs |
| GFF3 | Yes | Feature rows | `annotation.gxf.stats.v1`, `annotation.gxf.normalize.v1`, `annotation.gene-position.v1`, `annotation.sequence.extract.v1`, `genome.gene-density.v1` | Strict nine-column parsing, gzip input, normalization, coordinate tables, reference-guided FASTA extraction, and sliding-window feature density |
| GTF | Yes | Feature rows | `annotation.gxf.stats.v1`, `annotation.gxf.normalize.v1`, `annotation.gene-position.v1`, `annotation.sequence.extract.v1`, `genome.gene-density.v1` | GTF attributes can be normalized to GFF3 and used for coordinate, sequence-extraction, or feature-density analysis |
| VCF | Yes | Variant rows | `variant.stats.v1`, `variant.filter.v1`, `variant.normalize.v1` | Plain, gzip, and BGZF input; VCF text output; no BCF |
| SAM | Yes | Alignment rows | `alignment.qc.v1` | Text SAM flag and mapping QC; plain or gzip |
| BAM | Magic bytes only | Binary metadata | None | `recognized-unsupported` |
| BCF, CRAM | Magic bytes only | Binary metadata | None | `recognized-unsupported` |
| HDF5, H5AD, LOOM | Signature plus extension hints | Binary metadata | None | Domain import is planned |
| RDS | Magic bytes only | Binary metadata | None | `recognized-unsupported` |
| PDB | Recognized text structure | 3D GUI and PNG snapshot | `structure.pdb.summary.v1`, `structure.sequence.extract.v1`, `structure.contact-map.v1`, `structure.geometry.v1`, `structure.superpose.v1` | Plain, gzip, or BGZF; first-model coordinate analysis except all-model PDB summary; explicit pLDDT remains opt-in |
| mmCIF | Recognized text structure | 3D GUI and PNG snapshot | `structure.mmcif.summary.v1`, `structure.sequence.extract.v1`, `structure.contact-map.v1`, `structure.geometry.v1`, `structure.superpose.v1` | Plain, gzip, or BGZF; supported `_atom_site` loops; first-model derived analysis except all-model summary |
| BLAST tabular | Content-validated tabular records | Bounded text | `similarity.blast.parse.v1`, `similarity.reciprocal.v1` | Default outfmt 6 and declared outfmt 7 fields; reciprocal analysis requires forward and reverse files |
| BLAST XML | XML1 root detection | Bounded text | `similarity.blast.parse.v1`, `similarity.reciprocal.v1` | Legacy XML1 iterations and HSPs; XML2 is not claimed |
| Protein-domain tables | InterProScan TSV or HMMER domtblout content | Bounded text | `protein.domain.parse.v1` | Deterministic parsing of coordinates, scores, accessions, sources, and available annotations |
| Newick | Balanced tree content ending in `;` | Bounded text | `phylogeny.tree.transform.v1` | Parses one tree, reports topology metrics, supports deterministic relabel/reroot output, and writes `.nwk` |
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
可进行矩形表达矩阵质控、标准化、PCA、样本/特征聚类、原生聚类热图和精确集合交集分析，蛋白 FASTA
可计算长度、组成、分子量、理论等电点、pH 7 电荷、芳香性、GRAVY 和消光系数；PDB 可生成结构摘要和显式 pLDDT
统计，PDB/mmCIF 均可提取坐标序列、计算接触、测量几何并按坐标身份做刚体叠合。两种格式
均可在 GUI 中进行有界 3D 预览并导出当前视角 PNG。GFF3、GTF 已支持
统计、规范化、坐标表、参考序列提取和滑动窗口特征密度。BLAST 表格和旧版 XML1
可解析命中并进行双向最佳命中分析，InterProScan TSV 和 HMMER domtblout 可解析蛋白结构域，
Newick 可统计拓扑并执行确定性的重命名、重定根和 `.nwk` 输出；BAM、BCF、CRAM、H5AD
等二进制格式仅识别，不会伪装成可用能力。

预览最多读取 200 条记录或 10 MiB 解压后内容。表格默认导出 CSV，也支持 TSV、
JSON、逐行对象 JSONL 和 XLSX。需要保留 VCF、BED、GFF3 等领域语义时，应保留
原始领域格式，不应把表格导出当作无损往返转换。
