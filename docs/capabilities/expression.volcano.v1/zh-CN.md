# 差异表达火山图

## 用途

从差异表达结果表在本地生成 SVG 火山图。

## 输入

包含有限 `log2FoldChange` 和 `padj` 列的 CSV。

## 参数

`--padj`、`--log2-fold-change` 和 `--max-points` 分别设置显著性、效应量与渲染上限。

## 示例

```text
linxira-bio expression volcano differential.csv volcano.svg --json
```

## 输出

SVG 图。红色和蓝色点满足设定的校正 P 值与倍数变化阈值。

## 结果解读

点表示效应量与校正后显著性，不能单独证明生物学因果关系。

## 注意事项

该图不执行差异表达统计，也不能代替实验设计审查。

## 运行时依赖

仅使用本地 Rust。统计估计仍由差异表达工作流负责。

## 引用

应引用生成输入结果表的差异表达统计方法。

## 故障排除

请导出具有准确列名的 CSV，并移除非有限数值。
