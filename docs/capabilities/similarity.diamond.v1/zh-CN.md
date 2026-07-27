# 本地 DIAMOND 搜索

## 用途

从参考 FASTA 建立隔离的 DIAMOND 蛋白数据库，并在本地运行 `blastp` 或 `blastx`。

## 输入

蛋白或待翻译核酸查询 FASTA，以及蛋白参考 FASTA。

## 参数

选择 `blastp` 或 `blastx`，设置线程、e-value、最多命中数和表格 outfmt 6/7。

## 输出

DIAMOND 表格结果、JSON 执行元数据和 Worker v2 artifact 哈希。

## 示例

```bash
linxira-bio similarity diamond proteins.fa reference.fa hits.tsv --mode blastp --threads 8 --json
```

## 结果解读

结合数据库完整性审查比对一致性、覆盖度、e-value 和分数。

## 注意事项

包装层会建立并删除临时数据库；不会下载蛋白数据库，也不会静默改变敏感度预设。

## 运行时依赖

需要托管环境中可发现的本地 DIAMOND 程序。

## 引用

引用 DIAMOND 及其版本、参考数据库来源与版本、搜索模式和阈值。

## 故障排除

审计 `diamond` 工具；程序不在 `PATH` 时配置 `LINXIRA_BIO_DIAMOND`。
