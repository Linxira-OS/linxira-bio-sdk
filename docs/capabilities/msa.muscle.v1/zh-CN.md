# MUSCLE 多序列比对

## 用途

在本地运行 MUSCLE 5，生成可复用的 FASTA 多序列比对。

## 输入

包含待比对核酸或蛋白序列的本地 FASTA。

## 参数

选择标准 `align` 或大数据集 `super5` 模式，并设置线程数。

## 输出

比对后的 FASTA、JSON 执行元数据和 Worker v2 输入输出哈希。

## 示例

```bash
linxira-bio msa muscle sequences.fa alignment.fa --mode align --threads 4 --json
```

## 结果解读

在下游推断前检查比对长度、gap 分布、序列覆盖和模型假设。

## 注意事项

多序列比对不是系统发育树；包装层不裁剪列、不推树，并拒绝覆盖已有输出。

## 运行时依赖

需要本地 MUSCLE 5 程序。

## 引用

引用 MUSCLE、版本和模式、输入序列来源，以及下游裁剪或推断方法。

## 故障排除

审计 `muscle`；程序不在 `PATH` 时配置 `LINXIRA_BIO_MUSCLE`。
