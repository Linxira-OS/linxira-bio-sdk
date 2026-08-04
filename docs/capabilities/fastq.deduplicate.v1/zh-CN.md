# fastq.deduplicate.v1

## 用途

在本地删除 FASTQ 中的精确重复读段，并可将严格提取的 UMI 纳入重复键。

## 输入

- 一个纯文本、gzip 或 BGZF FASTQ 文件。

## 参数

- `output`：必填 FASTQ 输出路径；不会覆盖已有文件。
- `header_umi_delimiter`：取读段标识符中最后一个分隔符后的后缀作为 UMI。
- `sequence_prefix_umi`：取序列开头指定数量的碱基作为 UMI，其余部分作为插入
  序列。两个 UMI 来源最多选择一个。

## 输出

- 规范化四行 FASTQ，每个精确键保留第一条读段。
- JSON 摘要：输入、输出、重复读段数、碱基数、策略和警告。

## 示例

```bash
linxira-bio fastq deduplicate reads.fastq.gz unique.fastq --json
linxira-bio fastq deduplicate reads.fastq umi-unique.fastq \
  --header-umi-delimiter : --json
```

## 结果解读

序列比较不区分大小写。无 UMI 时以序列为键；启用 UMI 时，UMI 和插入序列都
必须完全一致才判为重复。

## 注意事项

这是单文件精确去重，不做 UMI 纠错、近似 UMI 聚类、共识序列或最高质量代表
选择、双端同步，也不按比对坐标识别片段重复。

## 运行时依赖

纯本地 Rust 能力，不依赖 Python、R、Java 或外部 FASTQ 工具。

## 引用

保留去重策略、UMI 提取规则、输入哈希、输出路径和结果 JSON。不同建库方案的
重复率不能直接比较。

## 故障排除

- 读名缺失 UMI 或 UMI 为空时会直接失败，避免静默使用错误键。
- 序列前缀 UMI 后必须至少保留一个插入碱基。
- 输出路径已存在时请换用新路径。
