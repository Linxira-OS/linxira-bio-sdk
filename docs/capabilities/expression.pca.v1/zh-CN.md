# 表达矩阵 PCA

## 用途

以样本为观测、表达特征为变量，执行确定性的主成分分析。

## 输入

完整的本地 CSV/TSV 矩阵，特征标识必须唯一，并至少包含两个样本和一个非恒定特征。

## 参数

- `--components N`：请求的主成分数（默认 2）。
- `--scale`：非恒定特征再按样本标准差缩放（worker 契约参数名 `scale_features`）。
- `--backend auto|rust|python|r`：实现后端。`rust`（及缺省）运行原生引擎；
  `python` 和 `r` 经 worker 路由到 benchmark 包
  （`org.linxira.benchmark-python` / `org.linxira.benchmark-r`），两者按同一
  确定性幂迭代（最大绝对分量取正）复刻实现，结果落在 1e-6 一致性容差内。
  `auto` 查询 `runtime-preferences.json`：命中非 rust 后端时输出
  `backend_from_preferences` 警告。
- `--json`：输出完整结果封装。

## 输出

JSON 包含样本得分、特征值、解释方差比例，以及各主成分最强的正负特征载荷。

## 示例

```bash
linxira-bio expression pca matrix.tsv --components 2 --scale --json
```

## 结果解读

用得分图检查主要样本差异，用载荷识别对各坐标轴贡献较大的特征。

## 注意事项

PCA 属于探索性分析，不能单独证明生物学分组或显著性。缺失值、重复特征和非有限值会被拒绝，
本地数值分析最多处理 1000 万个矩阵单元格。

## 运行时依赖

- `rust`（默认）：中心化协方差算子和特征求解器均为本地 Rust 实现。
- `python` / `r`：对应的 benchmark 包——Python 需 `numpy`，R 无需额外包
  （base R 的 `sin`/`cos` 幂迭代）。均无网络访问。

## 引用

应引用 PCA，以及生成待分析矩阵时所用的上游标准化方法。

## 故障排除

- 无法解析全部主成分时移除恒定特征；特征数值尺度不可直接比较时启用缩放。
- `unknown --backend value`：该参数只接受 `auto`、`rust`、`python`、`r`。
