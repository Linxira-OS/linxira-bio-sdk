# 批次效应校正

## 用途

用 ComBat 参数化经验贝叶斯方法去除表达矩阵中的技术批次效应（处理日期、
实验室、建库批次），同时保留生物学协变量，使下游 PCA、聚类与差异比较
反映生物学而非批次结构。

## 输入

两个文件。一个特征 × 样本的表达矩阵（CSV/TSV，第一列为特征 id），须为
归一化或 log 变换后的值——对原始计数直接做 ComBat 在统计上不成立。一个
样本表（CSV/TSV，第一列为样本 id），其 id 与矩阵列完全一致，含批次列及
需要保留的可选数值型或二分类协变量。

## 参数

`--sample-table`（必填）指向样本表。`--batch-column` 选择批次列（默认
`batch`）。`--method combat` 为唯一方法（参数化经验贝叶斯）。
`--backend auto|python|r` 选择实现：NumPy（`auto` 默认）或 base R。两者
实现同一规格，输出校正矩阵逐字节一致。

## 输出

与输入同方向、同特征/样本 id 的校正矩阵 TSV，以及 JSON 摘要：方法、
特征/样本数、批次计数、保留的协变量列、经验贝叶斯最大迭代次数，以及
各批次先验（gamma bar、tau squared、a、b）。

## 示例

```bash
linxira-bio expression batch-correct expr.csv expr-corrected.tsv \
  --sample-table samples.csv --batch-column batch --json
linxira-bio expression batch-correct expr.csv expr-corrected.tsv \
  --sample-table samples.csv --backend r --json
```

## 结果解读

校正后各批次均值对齐（log 尺度数据残余离散 ≲ 0.1），各特征的协变量效应
在噪声范围内保持不变。经验贝叶斯先验把每批次每特征的调整量向跨特征分布
收缩，在小批次下稳定估计。若校正后仍见批次签名，通常是模型无法分离的
批次-协变量混杂；设计矩阵秩亏时能力直接拒绝，而不是返回被混杂的输出。

## 注意事项

每个批次至少两个样本。残余方差为零的特征（跨样本恒定）被拒绝，缺失值
与样本 id 不匹配同样被拒绝。校正值不是计数；标准化只在校正前做，不在
校正后重做。与批次共线的协变量会被秩检查拒绝，而不是被部分吸收。

## 运行时依赖

Python 后端：锁定的 NumPy 环境（`LINXIRA_BIO_WORKFLOW_PYTHON`）。R 后端：
base R 加项目库中的 jsonlite（`LINXIRA_BIO_WORKFLOW_R_LIBRARY`）。无网络
访问。

## 引用

Johnson, W. E., Li, C., & Rabinovic, A. (2007). Adjusting batch effects in
microarray expression data using empirical Bayes methods. Biostatistics
8(1), 118–127. doi:10.1093/biostatistics/kxj037

## 故障排除

"batch design matrix is rank deficient" 表示协变量与批次共线（例如某批次
只处理了一种条件）——要么去掉该协变量，要么补齐平衡设计。"degenerate
empirical-Bayes prior" 或不收敛通常意味着某批次样本太少、方差无法估计，
合并或剔除该批次。后端输出不一致是缺陷，不是可以放宽的容差。
