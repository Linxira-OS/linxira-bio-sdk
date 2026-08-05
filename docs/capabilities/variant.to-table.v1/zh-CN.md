# VCF 转表格

## 用途

将 VCF 变异记录转换为 TSV 表格，包含固定的 CHROM、POS、ID、REF、ALT、QUAL、
FILTER、INFO 列以及每个样本一列。

## 输入

一个有效的纯文本、gzip 或 BGZF 格式的 VCF 文件。不接受 BCF。

## 参数

输入和输出路径为必需参数。`--json` 返回标准分析结果封装。

## 输出

一个带表头的 TSV 文件。前八列为固定 VCF 字段。若 VCF 表头声明了样本，则每个
样本追加一列，列名为样本名称。样本列包含完整的 FORMAT:values 字符串。

## 示例

```bash
linxira-bio variant to-table tests/fixtures/variant-stats/mixed.vcf output.tsv --json
```

## 结果解读

每行对应一条 VCF 记录。ALT 等位基因保留原始逗号分隔形式。INFO 为原始值。样本列为
原始 FORMAT:values 文本。

## 注意事项

该能力不做基因型验证、等位基因规范化、多等位记录拆分或 INFO 字段解析。它只是将
原始 VCF 列流转为表格布局。

## 运行时依赖

纯 Rust 本地流式能力，无外部依赖。

## 引用

VCF 字段语义遵循 GA4GH VCF 规范。

## 故障排除

表头或记录损坏时，按错误中的行号定位。BCF 请先用 bcftools 等成熟工具转换为 VCF。