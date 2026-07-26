# 简单电子 PCR

## 用途

根据引物对和参考 FASTA 在本机定位精确匹配扩增子。

## 输入

一个参考 FASTA，以及含 `id`、`forward`、`reverse` 列的 TSV。

## 参数

可设置最小、最大扩增子长度，并用 `--max-hits` 设置安全上限。

## 输出

TSV 包含引物 ID、序列 ID、1-based inclusive 起止坐标、扩增子长度和链方向，并返回摘要。

## 示例

```bash
linxira-bio primer epcr reference.fa primers.tsv amplicons.tsv --max-amplicon 5000 --json
```

## 结果解读

反向引物先取反向互补，再与同一参考序列上的正向引物组成扩增子。

## 注意事项

仅支持无简并符号的精确引物；结果不预测退火温度、二聚体或实验成功率。

## 运行时依赖

纯 Rust 本地运行，无需外部比对器或运行时。

## 引用

实现遵循常用电子 PCR 引物方向和坐标约定。

## 故障排除

确认引物表为制表符分隔，包含三个必需列，且引物只含 A/C/G/T/U。
