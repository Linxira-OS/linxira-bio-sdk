# 本地 BLAST+ 搜索

## 用途

从本地参考 FASTA 在隔离临时目录建立 BLAST+ 数据库，并运行核酸或蛋白相似性搜索。

## 输入

查询 FASTA 和参考 FASTA，二者均不上传。

## 参数

可选 `blastn`、`blastp`、`blastx`、`tblastn` 或 `tblastx`，并设置线程、e-value、最多命中数和表格 outfmt 6/7。

## 输出

BLAST 表格结果、JSON 执行元数据，以及 Worker v2 输入输出哈希。

## 示例

```bash
linxira-bio similarity blast query.fa reference.fa hits.tsv --program blastn --threads 4 --json
```

## 结果解读

结合查询和参考组成解读一致性、比对长度、e-value 与 bit score。

## 注意事项

搜索后删除临时数据库；不会下载参考数据库，并关闭 BLAST 使用情况上报。

## 运行时依赖

需要本地 NCBI BLAST+ 的 `makeblastdb` 和所选搜索程序；Windows 可使用已配置的原生程序或获准的 WSL 环境。

## 引用

引用 NCBI BLAST+、参考 FASTA 来源与版本、搜索程序和参数。

## 故障排除

先审计 `ncbi-blast`；仅为明确配置的程序设置 `LINXIRA_BIO_MAKEBLASTDB` 和对应的 `LINXIRA_BIO_BLASTN` 类变量。
