# fastq.trim.v1

## 用途

在本地裁剪 FASTQ 读段 3' 端低质量碱基，并写出新的 FASTQ 文件，供后续比对、
组装或再次质控使用。

## 输入

- 一个 FASTQ 文件：支持纯文本、gzip 或 BGZF。

## 参数

- `min_quality`：低于该 Phred 分数的尾端碱基会被移除。默认 20。
- `min_length`：裁剪后短于该长度的读段会被丢弃。默认 20。
- `quality_encoding`：`phred+33` 或 `phred+64`。默认 `phred+33`。
- `output`：必填 FASTQ 输出路径；不会覆盖已有文件。

## 输出

- 规范化四行 FASTQ 文件。
- JSON 摘要：输入/输出读段数、丢弃读段数、被裁剪读段数、输入/输出碱基数、
  质量裁剪碱基数和警告。

## 示例

```bash
linxira-bio fastq trim reads.fastq.gz reads.trimmed.fastq \
  --min-quality 20 --min-length 20 --quality-encoding phred+33 --json
```

## 结果解读

使用前对比 `input_read_count`、`output_read_count`、
`discarded_read_count`、`quality_trimmed_bases` 和 `output_bases`。
需要质量证据时，在裁剪前后分别运行 `fastq.qc.v1`。

## 注意事项

当前版本只做 3' 端阈值裁剪；不做滑窗裁剪、poly-G/poly-X 裁剪、UMI 处理、
双端同步或去重复。

## 运行时依赖

纯本地 Rust 能力，不需要 Python、R、Java 或外部 FASTQ 工具。

## 引用

FASTQ 质量裁剪是标准预处理方法。请保留命令、参数、输入哈希、输出路径和结果
JSON，方便复现。

## 故障排除

- 如果所有读段都被丢弃，降低 `min_length` 或检查输入质量。
- 如果质量分数明显不对，确认 `quality_encoding`。
- 如果输出失败，换一个不存在的输出路径。
