# 分子描述符

## 用途

使用 RDKit 计算 SDF 分子记录的理化描述符：分子量、CLogP、TPSA、氢键供体/受体数、可旋转键数、环与芳香环数、形式电荷与分子式。

## 输入

包含一条或多条分子记录的 SDF 文件（以 `$$$$` 分隔）。

## 参数

除输入与输出路径外无需其他参数。

## 输出

TSV 描述符表，每个分子一行（`molecule_index` 加各描述符列）。JSON 输出报告 `molecule_count`、`descriptor_names` 与逐分子行。

## 示例

```bash
linxira-bio chemistry descriptors molecules.sdf descriptors.tsv --json
```

## 结果解读

分子量与分子式描述组成；CLogP 估计亲脂性；TPSA 与氢键数描述极性及渗透趋势；可旋转键与环数描述柔性与刚性。数值采用 RDKit 默认值（氢原子处理遵循 RDKit 标准解析器）。

## 注意事项

需要固定的 RDKit Python 环境（`requirements.lock`，Python 3.12）。结果取决于 RDKit 版本。RDKit 无法解析的分子会产生结构化错误。

## 运行时依赖

RDKit 2026.3.5 与 NumPy 2.5.2（Python 3.12），通过带哈希的 `requirements.lock` 安装。

## 引用

RDKit: Open-Source Cheminformatics Software（https://www.rdkit.org）。

## 故障排除

若分子解析失败，请检查 SDF 记录有效性（原子与键块）。确认包环境已安装：`pip install --require-hashes -r workflows/org.linxira.chemistry-descriptors-rdkit/requirements.lock`。
