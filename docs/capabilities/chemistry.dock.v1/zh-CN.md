# 分子对接

## 用途

用原生 AutoDock Vina 将准备好的小分子配体对接进准备好的受体，产出poses 构象与按
结合亲和力排序的表，用于本地结构导向分析。

## 输入

两个 PDBQT 文件：刚性受体与配体（刚性或可柔化）。准备是前置条件而非本能力的一部分——
配体用 Meeko（`mk_prepare_ligand.py`）准备，受体用 AutoDockTools 流程准备后再对接。

## 参数

搜索盒为必填：`--center-x/--center-y/--center-z` 与 `--size-x/--size-y/--size-z`
（单位埃；边长必须为正）。可选：`--seed N` 保证可复现、`--exhaustiveness`（默认 8，
越高越慢越彻底）、`--num-modes`（默认 9）、`--cpu N`（默认 1）。

## 输出

写出包含全部报告 poses（按排名排序）的对接后配体 PDBQT。JSON 报告原生工具溯源信息
及解析出的 pose 表：每个 mode 的排名、kcal/mol 亲和力、RMSD 下/上界，rank-1 亲和力
单列为 `best_affinity_kcal_per_mol`。

## 示例

```bash
linxira-bio chemistry dock receptor.pdbqt ligand.pdbqt docked.pdbqt \
  --center-x 11.7 --center-y -4.5 --center-z 0.25 \
  --size-x 20 --size-y 20 --size-z 20 --seed 42 --json
```

## 结果解读

Vina 打分是经验性的相对值：有效比较仅限同一配体在同一搜索盒中的 poses，或在完全相同
设置下对接的一组配体。应连同完整 pose 表一起报告 rank-1 亲和力；低排名 poses 描述能量
面而非等价备选。RMSD 界衡量的是与最优 mode 的距离，不是与实验真值的距离。

## 注意事项

打分不是实测结合常数，不得当作 Ki 或 Kd 解读。水分子、质子化状态与受体柔性不在本
固定受体流程内。若运行成功但 stdout 无 pose 表，会给出警告且 `docking.modes` 为空——
应视为不可解读的失败，而非零亲和力结果。

## 运行时依赖

原生 `vina` 可执行文件（环境审计 molecular-docking 类别；bioconda
`autodock-vina`；可经 `LINXIRA_BIO_VINA` 覆盖）。Meeko（bioconda `meeko`）作为准备
前置被探测。对接步骤本身不使用 Python、R 或 Java 运行时。

## 引用

报告对接结果时引用 Trott & Olson 2010（J. Comput. Chem. 31:455-461）与
Eberhardt 等 2021（J. Chem. Inf. Model. 61:3891-3898，AutoDock Vina 1.2.x）。

## 故障排除

先运行环境审计确认 `vina` 已安装。"File cannot be opened" 几乎总是输入不是 PDBQT。
若 Vina 成功退出但没有 pose 表，换更小的搜索盒或更高 exhaustiveness 重跑，并检查警告
指向的原始日志。
