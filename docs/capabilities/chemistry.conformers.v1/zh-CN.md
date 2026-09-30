# 构象生成

## 用途

用 RDKit ETKDGv3 在固定种子下为 SDF 分子生成三维构象，并以 MMFF94 对每个构象做能量
最小化，产出供分子对接与形状比较使用的结构系综。

## 输入

单个 SDF 文件（支持多记录）。记录可以是 2D 的；嵌入前会加氢，输出携带显式氢原子（
三维结构所需）。

## 参数

`--num-conformers`（默认 10，范围 1-1000）、`--seed`（默认 42；固定种子使每次运行
字节级一致）、可选 `--rms-prune F`（单位埃；与更早构象 RMSD 小于阈值的构象被丢弃，
因此返回数量可能低于请求值）。

## 输出

多记录 SDF，每个构象一条记录，携带 `_ConformerIndex` 与 `mmff_energy_kcal_mol`
属性。JSON 结果信封汇总每个分子的构象数与 MMFF94 能量范围（kcal/mol），以及产物
路径、大小与 SHA-256。

## 示例

```bash
linxira-bio chemistry conformers ligands.sdf ligand-conformers.sdf --num-conformers 20 --seed 42 --json
```

## 结果解读

能量是 MMFF94 力场值（kcal/mol），适用于同一分子内构象排序——不能用于跨分子比较。
能量最低的构象不一定是生物活性 pose；用对接在构象间做选择。加剪枝阈值后能量跨度大
说明分子柔性强、系综本身很重要。

## 注意事项

需要 worker 选定的固定版本 RDKit Python 3.12 环境
（`LINXIRA_BIO_WORKFLOW_PYTHON`）；pack 绝不改动全局环境。原子类型不在 MMFF94 参数
覆盖范围内的分子会以结构化错误失败，而不是悄悄跳过最小化。可复现性依赖种子：换
种子即换系综。

## 运行时依赖

pack `requirements.lock` 锁定的 Python 3.12 环境（RDKit 2026.3.5、NumPy 2.5.2；
安装由环境能力门控）。不调用任何原生工具。

## 引用

报告构象系综时引用 Ebejer 等 2012（J. Chem. Inf. Model. 52:1146-1158，ETKDG）与
Halgren 1996（J. Comput. Chem. 17:490-519，MMFF94），以及描述符能力文档中的 RDKit
引用。

## 故障排除

"MMFF94 has no parameters" 表示分子含力场不覆盖的原子类型（常见于金属）；此类输入
无法最小化。分子合法却得到零构象通常是嵌入窗口受限——去掉 `--rms-prune` 或换种子
重试。
