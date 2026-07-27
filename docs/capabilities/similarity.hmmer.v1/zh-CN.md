# 本地 HMMER Profile 搜索

## 用途

在本地运行 `hmmsearch` 或 `hmmscan`，并保留确定性的结构域表格输出。

## 输入

HMMER profile 或已 press 的 profile 数据库，以及与所选模式匹配的序列 FASTA。

## 参数

选择 `hmmsearch` 或 `hmmscan`，设置 CPU 线程和报告 e-value。

## 输出

HMMER `--domtblout` 文本、JSON 执行元数据和 Worker v2 哈希。

## 示例

```bash
linxira-bio similarity hmmer profile.hmm proteins.fa domains.domtblout --mode hmmsearch --json
```

## 结果解读

结合结构域坐标、独立 e-value、分数、profile 覆盖和模型来源解读。

## 注意事项

本包装层不建立、press、下载或分发 profile 数据库；`hmmscan` 需要为该模式准备的数据库。

## 运行时依赖

需要本地 HMMER `hmmsearch` 或 `hmmscan`；Windows 通常使用获准的 WSL provider，除非已配置兼容程序。

## 引用

引用 HMMER、profile 数据库版本、模型 accession、搜索模式和阈值。

## 故障排除

审计 `hmmer`；仅在程序不在 `PATH` 时配置 `LINXIRA_BIO_HMMSEARCH` 或 `LINXIRA_BIO_HMMSCAN`。
