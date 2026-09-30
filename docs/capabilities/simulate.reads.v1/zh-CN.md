# 读段模拟

## 用途

以种子化生成从参考 FASTA 合成确定性单端或双端 FASTQ 读段——为比对基准、变异检测演练
与 GPU 一致性参照提供可复现输入。

## 输入

一个参考 FASTA（明文或 gzip）。短于读长的序列会被采样器跳过。

## 参数

`--read-length N`（默认 150）；用 `--coverage C`（百分比，默认 30）或 `--read-count N`
指定产量；`--paired` 输出相距 `--fragment-length`（默认 300）的 mate pair；
`--error-rate` 设定逐碱基替换概率（百分比，默认 0）；`--seed`（默认 42）；
`--quality-char` 一个可打印 ASCII 质量符号（默认 `I`，即 Q40）。

## 输出

每读一条记录的 FASTQ（双端模式带 `/1`、`/2` 后缀；header 携带来源位置，`_revpos`
标记反向互补读段）与 JSON 汇总：种子、是否双端、参考计数、产出读数、请求与实测
覆盖度、错误率。

## 示例

```bash
linxira-bio simulate reads reference.fa reads.fq --read-length 150 --coverage 30 --paired --error-rate 0.1 --seed 42 --json
```

## 结果解读

实测覆盖度会与请求值略有偏差，因为按读数而非碱基平衡计数。错误模型只有替换；读段
比真实仪器数据干净，对比 QC 流水线时要记住这一点。种子加选项即复现契约。

## 注意事项

无插入缺失、无质量衰减、无偏倚模型——完全均匀采样。短于读长的序列不参与采样；只含
短序列的参考会被拒绝。片段落点在序列末端被截断。

## 运行时依赖

除 CLI 外无依赖——纯 Rust，xoshiro256** 经 SplitMix64 播种。

## 引用

未复现任何外部模拟器（对比 art/wgsim）；当模拟读段支撑已发表的对比时，按版本引用
本能力。

## 故障排除

"no reference sequence is at least read_length long"：降低 `--read-length` 或提供更
长的参考。读数与覆盖度请求差距大：参考太小——用 `--read-count` 精确指定。
