# 序列随机化

## 用途

使用 Fisher-Yates 洗牌算法随机化 FASTA 文件中序列的顺序，通过用户指定的种子保证可重复性。

## 输入

包含一条或多条序列的纯文本或 gzip 格式 FASTA 文件。

## 参数

`--seed` 设置随机种子以确保确定性洗牌（必填）。

## 输出

序列相同但顺序随机化的 FASTA 文件。JSON 结果包含输入序列数和使用的种子。

## 示例

```bash
linxira-bio sequence shuffle input.fa shuffled.fa --seed 42 --json
```

## 结果解读

验证输出序列数与输入一致。给定相同种子时洗牌结果是确定性的。

## 注意事项

仅随机化序列顺序，序列内容不变。内存使用量与序列总数成正比。

## 运行时依赖

仅需本地 Rust；无需 Python、R、Java 或外部可执行文件。

## 引用

Fisher RA, Yates F. Statistical tables for biological, agricultural and
medical research. 1938.

## 故障排除

确保输入为有效的 FASTA 格式。已有输出文件不会被覆盖。