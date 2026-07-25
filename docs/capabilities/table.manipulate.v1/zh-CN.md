# table.manipulate.v1

## 用途

在本地过滤和重塑矩形 CSV/TSV 生物表格，避免临时编写脚本。适用于样本表、基因列表、计数表、注释导出表，以及需要选择列、删除列、过滤行、跳过行、限制输出行数或转换分隔符的文件。

## 输入

- 一个带表头行的 CSV 或 TSV 表格。
- 支持普通文本和 gzip 压缩输入。
- 在可判断时识别 `.csv`、`.tsv`、`.tab`、`.csv.gz`、`.tsv.gz`、`.tab.gz`、`.bgz` 和 `.bgzip` 扩展名。

## 参数

- `--select-column NAME`：保留指定列，可重复并按给定顺序输出。
- `--drop-column NAME`：删除指定列，可重复。
- `--filter-column NAME`：行过滤使用的列。
- `--filter-op equals|contains|non-empty`：字符串过滤操作。
- `--filter-value VALUE`：`equals` 和 `contains` 的匹配值。
- `--skip-rows N`：跳过表头之后的前 N 行数据。
- `--limit N`：最多写出 N 行数据。
- `--delimiter csv|tsv`：手动指定输入分隔符。
- `--output-delimiter csv|tsv`：手动指定输出分隔符。
- `--json`：输出结构化结果信封。

## 输出

- 一个新的 CSV 或 TSV 文件；不会覆盖已有文件。
- 摘要包含输入/输出行数、跳过行数、过滤行数、输入/输出列数、保留列、删除列、分隔符和警告。
- Worker v2 任务还会返回表格 artifact，包含格式、媒体类型、大小和 SHA-256。

## 示例

```bash
linxira-bio table manipulate counts.tsv selected.tsv --select-column gene_id --select-column sample_a --limit 100 --json
```

```bash
linxira-bio table manipulate annotations.csv genes.tsv --filter-column type --filter-op equals --filter-value gene --output-delimiter tsv --json
```

## 结果解读

用 `input_rows`、`skipped_rows`、`filtered_rows` 和 `output_rows` 确认保留的行是否符合预期。用 `selected_columns` 和 `dropped_columns` 在进入下游分析前确认列投影是否正确。

## 注意事项

- 过滤是字符串过滤，每次运行支持一个过滤条件。
- 选择列和删除列互斥。
- join、merge、分组统计、表达矩阵 QC、稀疏表格和统计建模不属于这个能力。
- 如果下游工具需要 FASTA、FASTQ、BED、GFF/GTF、VCF、SAM/BAM 或 PDB 语义，应优先保留领域格式。

## 运行时依赖

在 Rust core 中本地运行。不需要 Python、R、Java、BLAST、DIAMOND、Docker、WSL、GPU 或网络访问。

## 引用

未引入外部科学方法。该能力实现确定性的分隔表读写和行列选择。

## 故障排除

- 如果无法判断分隔符，传入 `--delimiter csv` 或 `--delimiter tsv`。
- 如果提示找不到列，检查表头拼写、空格或重复列名。
- 如果输出文件已存在，换一个新路径，或确认后先删除旧文件再运行。
