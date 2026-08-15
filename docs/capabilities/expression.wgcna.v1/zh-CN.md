# WGCNA 共表达网络

## 用途

使用 WGCNA R 包从表达矩阵构建加权基因共表达网络，识别共表达基因模块。

## 输入

CSV 或 TSV 表达矩阵，行为基因、列为样本。数值必须为非负有界。

## 参数

`--min-expression` 每基因最低表达阈值（默认 1）。`--min-samples` 满足阈值的最低样本数（默认 3）。
`--min-module-size` 最小模块大小（默认 30）。`--merge-cut-height` 模块合并阈值（默认 0.25）。
`--network-type` signed、unsigned 或 signed hybrid（默认 signed）。`--power` 软阈值幂次（0 = 自动检测）。
`--no-log-transform` 跳过 log2(x+1) 转换。`--threads` 线程数。

## 输出

JSON 结果包含产物路径：`module-assignments.csv`（基因-模块对应）、`module-eigengenes.csv`（样本特征基因）、
`module-summary.csv`（模块大小）和 `scale-free-fit.csv`（软阈值拟合指标）。

## 示例

```bash
linxira-bio expression wgcna expression.tsv results.json --min-module-size 30 --threads 4 --json
```

## 结果解读

验证无标度拓扑拟合度（建议 R² > 0.8）。在解读生物学意义前先审查模块大小和特征基因特征。

## 注意事项

需要安装 R 和 WGCNA 包。大矩阵消耗大量内存。该分析属于探索性分析，不能证明因果关系。

## 运行时依赖

R 4.3+，以及 WGCNA、dynamicTreeCut、fastcluster 包。

## 引用

Langfelder P, Horvath S. WGCNA: an R package for weighted correlation network
analysis. BMC Bioinformatics. 2008;9:559.

## 故障排除

若自动检测软阈值失败，手动指定 `--power`。若无模块被检出，降低 `--min-module-size`。
确保表达矩阵无缺失值或负值。