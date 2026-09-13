# curve.fit.v1

对两列表 CSV/TSV（x = 浓度或底物，y = 响应或速率）做本地确定性曲线拟合，
支持三种模型：用于 IC50 与 ELISA 标准曲线的四参数逻辑斯蒂（4PL）、用于
底物饱和动力学的米氏方程（Michaelis-Menten）、以及 Lineweaver-Burk
双倒数线性化交叉验证。求解器为固定迭代次数并带步长阻尼的 Gauss-Newton
循环；无外部拟合依赖，同一输入的输出完全可复现。

## 用途

把酶标仪与动力学数据表转成可报告的曲线参数——4PL 的 IC50/EC50、上下渐近线
与斜率因子，直接拟合的 Vmax 与 Km，以及 Lineweaver-Burk 交叉验证——并给出
逐点残差与近似参数标准误用于质量控制。

## 输入

- 一个 CSV 或 TSV 表（可 gzip），含表头行且至少两列；无论表头名称如何，
  取前两列分别作为 x 与 y。
- 缺失单元格（`NA`、`nan`、`.`）与非有限值会被拒绝；请先删除或插补这些行。

## 参数

- `--model 4pl|michaelis-menten|lineweaver-burk`（默认 `4pl`）。
- `--max-iterations N`（默认 200）：非线性模型的 Gauss-Newton 迭代上限。
- `--tolerance F`（默认 1e-10）：相对步长收敛阈值。
- `--json`：输出标准结果信封。

## 输出

- `model`：拟合的模型名。
- `point_count`：参与拟合的点数。
- `parameters`：带名字的参数估计及近似标准误——4PL 为 `a`/`b`/`c_ic50`/`d`
  （`c_ic50` 即拐点/IC50），米氏为 `vmax`/`km`，Lineweaver-Burk 为
  `slope`（= Km/Vmax）/`intercept`（= 1/Vmax）及换算出的 `vmax`/`km`。
- `r_squared`、`rmse`：拟合优度；Lineweaver-Burk 的 `r_squared` 描述线性化
  (1/S, 1/v) 回归，`rmse` 以原始 (S, v) 单位给出。
- `residuals`：逐点 `x`、`y`、`fit` 与 `residual`（y - fit）。
- `iterations`、`warnings`：求解器迭代次数与数据质量提示。

## 示例

```bash
linxira-bio curve fit dose-response.csv --model 4pl --json
linxira-bio curve fit kinetics.tsv --model michaelis-menten --json
linxira-bio curve fit kinetics.tsv --model lineweaver-burk --json
```

## 结果解读

- 4PL：`a` 是零浓度响应，`d` 是饱和响应；`c_ic50` 是曲线中点浓度，应落在
  实测浓度范围内；`b` 是斜率因子（类似 Hill 陡度）。
- 米氏动力学：只有当底物范围跨越 Km 上下并接近饱和时 `km` 才可靠；否则
  Vmax 与 Km 强相关，标准误增大。
- Lineweaver-Burk 是线性化交叉验证：干净数据上换算的 `vmax`/`km` 应与直接
  米氏拟合一致；系统性不一致说明误差结构被倒数变换放大。
- 参数标准误来自残差方差缩放的 (J^T J) 正规矩阵之逆，是局部渐近近似。

## 注意事项

- 4PL 与米氏模型要求 x（浓度/底物）为正且至少 4 个有效点；Lineweaver-Burk
  要求至少 2 个 x、y 均非零的点，取值为 0 的点会被剔除并给出 warning。
- 只取一个 `y` 列；复孔请预先平均，或按行逐点提供（每行即一个拟合点）。
- Gauss-Newton 求解器使用固定初值（4PL：a = min y、d = max y、c = x 的几何
  均值、b = 1；米氏：Vmax = max v、Km = 最接近半最大的底物浓度），达到迭代
  上限时带 warning 停止，不切换其他算法。
- 仅供科研使用，不得用于临床剂量决策。

## 运行时依赖

- 仅 Rust 引擎本身；不涉及 Python、R 或原生工具。

## 引用

- Seber, G. A. F. & Wild, C. J. (2003). Nonlinear Regression. Wiley.
- Michaelis, L. & Menten, M. L. (1913). Die Kinetik der Invertinwirkung.
  Biochemisches Zeitschrift, 49, 333–369.
- Lineweaver, H. & Burk, D. (1934). The determination of enzyme dissociation
  constants. Journal of the American Chemical Society, 56, 658–666.

## 故障排除

- "requires positive concentrations"：4PL 或米氏表中存在 x 为 0 或负值的行；
  删除零稀释行或改用其他模型。
- "requires at least 4 valid points"：增加稀释点，或改用
  `--model lineweaver-burk`（最少 2 个非零点）。
- "lineweaver-burk intercept is zero"：速率从未接近饱和；请直接用
  `--model michaelis-menten` 拟合。
- 迭代上限或奇异正规矩阵的 warning 表示参数处于目标函数的平坦/退化区域；
  拓宽浓度范围或检查离群点。
