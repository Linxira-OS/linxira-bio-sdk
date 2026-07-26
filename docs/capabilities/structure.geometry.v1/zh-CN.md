# 结构几何测量

## 用途

从本地 PDB 或 mmCIF 坐标测量一个原子距离、夹角或扭转角。

## 输入

一个坐标文件以及恰好两个、三个或四个原子选择器。

## 参数

重复使用 `--atom CHAIN/RESIDUE/ATOM` 或 `--atom MODEL/CHAIN/RESIDUE/ATOM`。

## 输出

返回测量类型、所选原子身份、数值和单位。

## 示例

```bash
linxira-bio structure geometry structure.pdb --atom A/1/N --atom A/1/CA --atom A/1/C --json
```

## 结果解读

两个选择器得到埃单位距离，三个得到角度，四个得到扭转角度。

## 注意事项

仅测量给定坐标，不判断立体化学合理性或化学键有效性。

## 运行时依赖

纯本地 Rust；不需要 Python、R、Java、网络或外部可执行文件。

## 引用

采用标准欧氏向量夹角和有符号扭转角公式。

## 故障排除

多模型文件应明确模型编号，并确认每个选择器唯一对应一个原子。
