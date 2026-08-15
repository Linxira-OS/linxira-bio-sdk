# 生存分析

## 用途

在包含生存时间与事件列以及分组列的队列表上拟合 Cox 比例风险模型，并报告各组的风险比（含置信区间）、p 值与 Kaplan-Meier 摘要。仅供研究使用。

## 输入

CSV 或 TSV 队列表，包含生存时间（数值）、事件指示（0/1）与分组变量列。

## 参数

- `--time-column <列>`（必填）：生存时间列。
- `--event-column <列>`（必填）：事件指示列（0/1）。
- `--group-column <列>`（必填）：分组/协变量列。
- `--reference-level <水平>`（必填）：风险比的参考组水平。

## 输出

`cox-results.csv`（项、系数、风险比、标准误、统计量、p 值、95% 置信区间）与 `km-summary.csv`（各组 n、事件数、中位生存时间）。JSON 输出报告模型项、逐项行与各组 Kaplan-Meier 摘要。

## 示例

```bash
linxira-bio medical survival cohort.csv results/ --time-column time --event-column event --group-column treatment --reference-level control --json
```

## 结果解读

风险比为相对于参考水平的指数化系数；大于 1 表示事件风险更高。p 值检验系数是否为零。中位生存时间指组内一半个体发生事件的时间（可估计时）。

## 注意事项

仅供研究使用，不构成临床决策支持。模型假设比例风险；不支持除单一分组列以外的协变量。除标准 Cox 框架外不进行额外删失调整。结（ties）使用默认的 efron 近似。

## 运行时依赖

R 及项目隔离库中的 `survival`、`jsonlite`、`digest` 包（见 `dependencies.lock.json`；`Rscript scripts/bootstrap-survival-lib.R <库目录>`）。

## 引用

Therneau, T.M., & Grambsch, P.M. (2000). Modeling Survival Data: Extending the Cox Model. Springer.

## 故障排除

若模型无法收敛，请检查分组列是否恒定、事件水平是否单一或时间列是否非数值。确认参考水平出现在分组列中。
