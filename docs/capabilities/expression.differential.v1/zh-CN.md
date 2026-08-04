# 批量 RNA-seq 差异表达

## 用途

在本地对原始批量 RNA-seq 计数拟合两条件 DESeq2 模型，并生成可审计的差异表达表和归一化计数表。

## 输入

提供一个 CSV 或 TSV 原始整数计数矩阵和一个 CSV 或 TSV 样本元数据表。特征 ID 必须唯一，样本 ID 必须与计数列完全一致，两个条件各至少需要两个生物学样本。

## 参数

必填字段为 `output_directory`、`feature_id_column`、`sample_id_column`、`condition_column`、`reference_level` 和 `contrast_level`。`alpha` 默认 `0.05`，`min_total_count` 默认 `10`。

## 输出

原子写入的输出目录包含 `differential-expression.csv`、`normalized-counts.csv` 和 `result.json`。结果记录输入哈希、有效参数、R 与包版本、过滤计数和依赖锁哈希。

## 示例

```text
linxira-bio workflow run org.linxira.bulk-expression-deseq2 request.json output/result.json
```

在 schema v2 请求中使用 `expression.differential.v1`。

## 结果解读

应结合调整后 p 值、表达量、重复质量和明确的对比方向解读 log2 倍数变化。多重检验显著性不能单独证明生物学重要性或因果关系。

## 注意事项

当前版本仅支持 `~ condition` 的两个条件。不得输入 TPM、FPKM、百分比或已归一化计数；暂不支持批次项、配对设计、交互项和协变量。

## 运行时依赖

需要已测试的稳定 R 4.6.x 解释器，以及包含兼容 DESeq2、jsonlite、digest 和完整依赖的项目隔离包库。通过 `LINXIRA_BIO_WORKFLOW_R` 与 `LINXIRA_BIO_WORKFLOW_R_LIBRARY` 选择；工作流不会安装包或修改全局包库。

## 引用

引用 Love MI, Huber W, Anders S. Genome Biology 15, 550 (2014)，并记录实际 DESeq2 包版本。

## 故障排除

运行环境审计，确认所有声明包来自项目包库，检查整数计数和样本 ID 是否匹配，并确保输出目录尚不存在。
