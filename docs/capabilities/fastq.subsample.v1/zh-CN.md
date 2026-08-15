# FASTQ 下采样

## 用途

使用水库采样算法从 FASTQ 文件中按目标数量或比例随机下采样 reads，实现内存高效处理。

## 输入

包含一条或多条 reads 的纯文本或 gzip 格式 FASTQ 文件。

## 参数

`--target-count` 设置保留的精确 reads 数量。`--fraction` 设置保留比例（0.0 到 1.0）。
`--seed` 设置随机种子以保证可重复性（默认 42）。

## 输出

下采样后的 FASTQ 文件。JSON 结果包含输入 reads 数、输出 reads 数和使用的采样方法。

## 示例

```bash
linxira-bio fastq subsample input.fastq output.fastq --target-count 10000 --seed 42 --json
```

## 结果解读

验证输出 reads 数与请求的目标数量或比例一致。水库采样保证每条 read 被选中的概率相等。

## 注意事项

目标数量超过输入数量时会产生警告并输出全部 reads。按比例采样因整数取整为近似值。

## 运行时依赖

仅需本地 Rust；无需 Python、R、Java 或外部可执行文件。

## 引用

Vitter JS. Random sampling with a reservoir. ACM Trans Math Softw. 1985;11(1):37-57.

## 故障排除

确保输入为有效的 FASTQ 格式且 reads 记录完整。已有输出文件不会被覆盖。