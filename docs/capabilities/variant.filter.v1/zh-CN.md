# VCF 基础过滤

## 用途

在本机验证并流式执行基础 VCF 记录过滤，不改写通过的记录。

## 输入

一个有效的纯文本、gzip 或 BGZF VCF；不接受 BCF。

## 参数

可组合最低 QUAL、仅 PASS、重复指定的染色体白名单和最低 `INFO/DP`。

## 输出

输出保留原始表头和通过记录的 VCF，并返回各淘汰原因的计数。

## 示例

```bash
linxira-bio variant filter input.vcf filtered.vcf --min-qual 20 --pass-only --min-info-dp 10 --json
```

## 结果解读

缺失 QUAL 不通过最低 QUAL；缺失 `INFO/DP` 不通过最低深度。

## 注意事项

不支持样本 FORMAT 字段过滤、变异重校准、注释或临床解释。

## 运行时依赖

纯 Rust 本地流式运行，无需 htslib、Python、R 或 Java。

## 引用

QUAL、FILTER、CHROM 和 INFO 语义遵循 GA4GH VCF 规范。

## 故障排除

按错误中的 VCF 行号定位损坏字段；BCF 请先用成熟原生工具转换为 VCF。
