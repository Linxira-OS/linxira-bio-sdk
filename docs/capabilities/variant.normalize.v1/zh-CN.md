# 参考序列驱动的 VCF 规范化

## 用途

验证 REF，并将双等位小变异规范为最简、重复区域左对齐表示。

## 输入

一个 VCF 文本文件和完全匹配的参考 FASTA；两者均可为纯文本、gzip 或 BGZF。

## 参数

必须提供输入 VCF、参考 FASTA 和输出 VCF；`--json` 返回标准结果封装。

## 输出

输出规范化 VCF，并返回参考验证、表示改变和左对齐记录数。

## 示例

```bash
linxira-bio variant normalize input.vcf reference.fa normalized.vcf --json
```

## 结果解读

先按 contig 和坐标验证 REF，再执行公共前后缀最简化和 indel 左对齐。

## 注意事项

拒绝多等位、符号、断点和 spanning-deletion ALT；本能力不拆分等位基因，也不重映射基因型。

## 运行时依赖

纯 Rust 本地运行，无需 htslib 或外部规范化程序。

## 引用

小变异表示遵循常用最简表示和重复区域左对齐约定。

## 故障排除

参考 contig 名称、版本和坐标必须完全匹配；复杂记录请交给成熟原生流程处理。
