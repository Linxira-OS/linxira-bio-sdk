# 数据集检查

## 用途

根据文件内容和压缩签名识别本地生物数据，在分析前进行有上限的校验并生成安全预览。

## 输入

`inputs.file` 必须是一个本地普通文件。当前支持 FASTA、FASTQ、CSV、TSV、BED、
GFF3、GTF、VCF 和 SAM。BAM、ZIP、BCF、CRAM、H5AD、LOOM、HDF5、RDS、
PDB 和 mmCIF 只能识别，当前版本不能完整校验或执行导入。

## 参数

`max_preview_records` 默认为 200，`max_preview_bytes` 默认为 10 MiB；两者都必须是正整数。

## 输出

JSON 结果包含路径、大小、识别格式、压缩类型、支持状态、置信度、预览列与记录、警告和结构化错误。

## 示例

```bash
linxira-bio dataset inspect reads.fastq.gz --json
```

## 结果解读

只有当 `support` 为 `supported` 且 `errors` 为空时才能继续分析。内容识别结果优先于冲突的扩展名。

## 注意事项

预览是抽样结果，不能代替分析过程中的完整校验。程序不会解压 ZIP。缺少领域元数据时，
HDF5 子类型不一定能可靠区分。

## 运行时依赖

该能力内置于本地 Rust worker，不依赖 Python、R、Java、容器或网络服务。

## 引用

格式语义遵循公开的 FASTA/FASTQ 约定、UCSC BED、GFF3/GTF、VCF 4.x 和 SAM/BAM 规范。

## 故障排除

出现 `format-extension-mismatch` 时应核对文件来源并采用内容识别结果。出现
`recognized-unsupported-format` 时应使用维护中的外部工具或等待对应能力，不能强制导入。
