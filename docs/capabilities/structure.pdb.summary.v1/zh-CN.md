# PDB 结构摘要

## 用途

在本地把 PDB 固定列坐标记录解析为确定性摘要和可供后续渲染使用的原子数据，
无需 Python、Java 或外部可执行程序。

## 输入

一个至少包含一条 `ATOM` 或 `HETATM` 记录的可读 PDB 文件。程序按内容识别纯
文本、gzip 和 BGZF；不接受 mmCIF。

## 参数

输入路径为必需参数。`--alphafold-plddt` 会显式把聚合物原子的 B-factor 解释
为 AlphaFold pLDDT；`--json` 返回标准分析结果封装。

## 输出

返回模型、链、残基、聚合物原子、异质原子和元素计数，单位为埃的坐标边界，
B-factor 统计，模型与链摘要，带索引的残基，以及包含坐标、占有率、B-factor、
元素、替代位置和残基身份的原子记录。显式 AlphaFold 模式还会返回逐残基
pLDDT 和四档置信度计数。

## 示例

```bash
linxira-bio structure pdb tests/fixtures/structure-pdb-summary/alphafold-style.pdb --alphafold-plddt --json
```

## 结果解读

用 `atoms[].residue_index` 关联 `residues[]`；用 `bounds.center` 和
`bounds.span` 设置相机取景。pLDDT 大于等于 90 为极高置信度，70 至 90 为可信，
50 至 70 为低置信度，小于 50 为极低置信度。它描述模型置信度，不等于实验验证。

## 注意事项

PDB 内容本身不能证明 B-factor 存放的是 pLDDT，因此程序绝不会自动启用这种
解释。本能力不解析 mmCIF 或 PAE，不推断化学键，不展开生物学组装，不解析替代
位置，不做结构比对、图像渲染或结构预测。

原生 GUI 可以独立载入 PDB/mmCIF 坐标、推断仅用于显示的键，并把当前视角导出为
PNG；这些显示行为不属于本分析能力的结果契约。

## 运行时依赖

这是纯本地 Rust 能力，不依赖 Python、R、Java、分子查看器或外部命令行工具；
除仓库已登记的序列化和 gzip 依赖外，不新增第三方包。

## 引用

PDB 列语义遵循 wwPDB 旧版 PDB 格式规范。AlphaFold pLDDT 解释遵循 AlphaFold
输出的置信度约定，并且只在调用者确认来源后启用。

## 故障排除

固定列记录损坏时按错误中的行号定位。mmCIF 可先用成熟结构工具转换，或保留到
后续原生解析器处理。晶体学 B-factor 不应使用 `--alphafold-plddt`。
