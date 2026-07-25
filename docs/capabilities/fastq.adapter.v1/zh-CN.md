# fastq.adapter.v1

## 用途

在本地从 FASTQ 读段 3' 端移除精确匹配的测序接头，并写出新的 FASTQ 文件。

## 输入

- 一个 FASTQ 文件：支持纯文本、gzip 或 BGZF。

## 参数

- `adapter`：单个接头序列。
- `adapters`：接头序列数组。只能在 `adapter` 和 `adapters` 中选一个。
- `min_overlap`：部分接头剪除所需的最小后缀重叠长度。默认 8。
- `min_length`：剪除后短于该长度的读段会被丢弃。默认 20。
- `output`：必填 FASTQ 输出路径；不会覆盖已有文件。

## 输出

- 规范化四行 FASTQ 文件。
- JSON 摘要：输入/输出读段数、丢弃读段数、被裁剪读段数、输入/输出碱基数、
  接头裁剪碱基数和警告。

## 示例

```bash
linxira-bio fastq adapter-trim reads.fastq.gz reads.no-adapter.fastq \
  --adapter AGATCGGAAGAGC --min-overlap 8 --min-length 20 --json
```

## 结果解读

`adapter_trimmed_bases` 和 `trimmed_read_count` 表示接头去除量。需要下游质量
证据时，在剪除后运行 `fastq.qc.v1`。

## 注意事项

当前版本只做 3' 端精确接头或部分接头剪除；不做容错匹配、自动接头发现、双端
同步、UMI 解析、质量裁剪或去重复。

## 运行时依赖

纯本地 Rust 能力，不需要 Python、R、Java、cutadapt、fastp 或其他外部 FASTQ 工具。

## 引用

接头剪除是标准读段预处理方法。请保留命令、参数、接头序列、输入哈希、输出路径
和结果 JSON。

## 故障排除

- 如果没有裁剪任何碱基，检查接头方向和 `min_overlap`。
- 如果太多读段被丢弃，降低 `min_length`。
- 如果输出失败，换一个不存在的输出路径。
