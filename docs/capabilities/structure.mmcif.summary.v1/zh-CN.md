# mmCIF 结构摘要

## 用途

在本地确定性汇总 mmCIF 原子坐标，无需外部工具。

## 输入

一个含受支持 `_atom_site` 循环的普通、gzip 或 BGZF mmCIF 文件。

## 参数

提供输入路径；使用 `--json` 返回标准结果信封。

## 输出

返回模型、链、残基、原子、聚合物原子、异质原子计数以及警告。

## 示例

```bash
linxira-bio structure mmcif-summary structure.cif --json
```

## 结果解读

计数覆盖文件中全部已解析模型和保留的备用构象。

## 注意事项

不展开生物学组装体，不覆盖全部 mmCIF 类别，也不推断化学键。

## 运行时依赖

纯本地 Rust；不需要 Python、R、Java、网络或外部可执行文件。

## 引用

字段解释遵循 wwPDB PDBx/mmCIF atom-site 数据模型。

## 故障排除

确认文件包含表格形式的 `_atom_site` 循环，并低于解压后输入上限。
