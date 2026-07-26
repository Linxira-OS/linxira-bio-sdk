# 自定义过度富集分析

## 用途

检验本地基因到术语关联在查询标识集合中的过度富集。

## 输入

查询标识列表，以及包含 `gene_id`、`term_id` 的 CSV/TSV 关联表；`term_name`、`namespace` 可选。

## 参数

可设置最小重叠、最多报告术语数和是否返回重叠标识；关联表全集就是背景。

## 输出

JSON 返回已映射/未映射查询、背景大小、单侧超几何 p 值、Benjamini-Hochberg 校正值、富集倍数和排序术语。

## 示例

```bash
linxira-bio enrichment custom genes.txt associations.tsv --include-genes --json
```

## 结果解读

在给定背景下，较小校正 p 值表示随机重叠解释较弱；富集倍数反映效应大小。

## 注意事项

结果依赖关联来源和背景定义，不能证明因果或临床意义。

## 运行时依赖

解析、精确计数、超几何上尾和多重检验校正均为本地 Rust。

## 引用

记录关联来源、背景定义、标识映射、筛选规则和校正方法。

## 故障排除

确保两个文件使用相同标识体系，并在解读前检查 `query_unmapped_count`。
