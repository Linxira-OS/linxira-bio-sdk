# 表达矩阵聚类

## 用途

使用确定性 k-means 分别对表达矩阵的样本和特征聚类。

## 输入

完整的本地 CSV/TSV 矩阵，特征标识唯一且所有数值有限。

## 参数

设置 `--sample-clusters`、`--feature-clusters` 和 `--max-iterations`。
默认按特征执行 z-score，可用 `--no-scale` 关闭。

## 输出

JSON 分别返回样本和特征的分配、到质心距离、簇大小、收敛状态、迭代次数和簇内平方和。

## 示例

```bash
linxira-bio expression cluster matrix.tsv --sample-clusters 2 --feature-clusters 4 --json
```

## 结果解读

结合独立实验元数据检查分配，并在描述模式前检查簇大小和到质心距离。

## 注意事项

聚类属于探索性分析。轴上项目少于请求簇数时会自动下调；结果受预处理和簇数影响，
本地数值分析最多处理 1000 万个矩阵单元格。

## 运行时依赖

确定性的最远点初始化和 k-means 均在本地 Rust 中运行。

## 引用

应引用 k-means，以及矩阵采用的标准化或变换方法。

## 故障排除

小矩阵应降低簇数；达到迭代上限时可提高 `--max-iterations`，并检查缩放和离群值。
