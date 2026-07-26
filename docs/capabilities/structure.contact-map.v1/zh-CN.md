# 残基接触图

## 用途

在本地 PDB/mmCIF 坐标中按距离阈值查找代表原子之间的残基接触。

## 输入

一个普通、gzip 或 BGZF PDB/mmCIF 坐标文件。

## 参数

`--cutoff` 指定埃单位阈值，`--atom` 指定代表原子，`--intra-chain-only` 排除链间接触。

## 输出

返回首模型、代表残基数、接触数、残基身份和距离。

## 示例

```bash
linxira-bio structure contact-map structure.cif --cutoff 8 --atom CA --json
```

## 结果解读

默认使用 CA、8 埃并包含链间接触；核酸可按需要选择 P。

## 注意事项

这是几何接触定义，不代表化学键或已经证实的生物学相互作用。

## 运行时依赖

纯本地 Rust；不需要 Python、R、Java、网络或外部可执行文件。

## 引用

距离为所选代表原子间的欧氏距离。

## 故障排除

选择结构中实际存在的原子名；结果最多返回 1,000,000 个接触。
