# trimAl 比对裁剪

## 用途

使用维护中的 trimAl 启发式方法裁剪本地多序列比对。

## 输入

输入本地 FASTA 多序列比对。

## 参数

模式可选 `automated1`、`gappyout`、`strict`、`strictplus` 或 `nogaps`。

## 输出

生成新的 FASTA 比对和 JSON 执行元数据；拒绝覆盖输入或已有输出。

```bash
linxira-bio msa trimal alignment.fa trimmed.fa --mode automated1 --json
```

## 示例

上面的命令运行默认自动启发式方法，并生成独立比对文件。

## 结果解读

建树前比较保留的比对长度和物种覆盖。

## 注意事项

裁剪会改变参与分析的位点集合。必须记录模式并检查保留列，不能默认裁剪后结果一定更好。

## 运行时依赖

需要 `PATH` 中的 trimAl，或设置 `LINXIRA_BIO_TRIMAL`；执行过程不经过 shell。

## 引用

引用 trimAl、版本、所选启发式方法和原始比对方法。

## 故障排除

审计 `trimal`；仅在程序不在 `PATH` 时设置 `LINXIRA_BIO_TRIMAL`。
