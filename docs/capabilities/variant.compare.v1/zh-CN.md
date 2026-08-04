# VCF 等位基因集合比较

## 用途

在本地比较两个 VCF，返回共有、仅左侧和仅右侧的变异等位基因，不上传数据。

## 输入

两个有效的 VCF 文本文件。程序按魔数识别纯文本、gzip 和 BGZF，并在比较前严格验证
header、记录、FORMAT 和 GT 语法。

## 参数

必须提供 `<left.vcf>` 和 `<right.vcf>`；`--json` 返回标准结果封装。

## 输出

JSON 返回三类集合计数，以及按 CHROM、POS、REF、ALT 稳定排序的明细表。每行标记为
`shared`、`left-only` 或 `right-only`。

## 示例

```bash
linxira-bio variant compare calls-a.vcf calls-b.vcf --json
```

## 结果解读

多 ALT 记录会拆成单独等位基因键，重复键会合并。序列等位基因会转为大写并分别化为
最简表示；CHROM 字符串和符号 ALT 字符串必须完全一致。

## 注意事项

本能力不比较样本、GT、定相、深度、质量、FILTER、INFO 或临床含义。仅做最简表示不能
合并重复区域中不同移位的 indel；需要这种等价性时，应先用同一参考序列规范化两份 VCF。

## 运行时依赖

纯 Rust 本地运行，无需 Python、R、Java、htslib 或外部程序。

## 引用

VCF 语法遵循 GA4GH VCF 规范。应记录两份文件使用的准确参考基因组版本和规范化方法。

## 故障排除

解释差异前，确认参考基因组版本、contig 命名、筛选条件和规范化策略一致。
