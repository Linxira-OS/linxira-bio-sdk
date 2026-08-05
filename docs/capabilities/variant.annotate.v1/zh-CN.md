# 变异注释

## 用途

使用参考数据库（如 Ensembl VEP）对 VCF 变异进行功能影响预测注释。

## 输入

包含待注释变异位点的 VCF 文件。

## 参数

`--database` 选择注释数据库（如 `GRCh38.99`）。

## 输出

添加了 INFO 字段注释的 VCF 文件。JSON 结果包裹原生工具执行元数据。

## 示例

```bash
linxira-bio variant annotate input.vcf output.vcf --database GRCh38.99 --json
```

## 结果解读

审查新增的注释信息，包括功能影响预测、基因后果和群体频率（如有）。

## 注意事项

需要本地可用的注释数据库。注释准确性取决于数据库版本和完整性。大型 VCF 文件可能需要较长处理时间。

## 运行时依赖

Ensembl VEP 或等效注释工具。设置 `LINXIRA_BIO_VEP` 可覆盖二进制路径。

## 引用

McLaren W, et al. The Ensembl Variant Effect Predictor. Genome Biol.
2016;17(1):122.

## 故障排除

若找不到注释数据库，请验证数据库路径和版本。确保 VCF contig 名称与注释数据库使用的参考基因组匹配。
已有输出文件不会被覆盖。