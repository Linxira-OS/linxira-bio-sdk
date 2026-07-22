# 表格导出

## 用途

通过 GUI、CLI、worker 和未来 SDK 共用的确定性实现导出结构化分析结果。

## 输入

输入必须是本地 JSON 对象、对象数组或二维数组。对于 `AnalysisResult` envelope，
表格格式使用其中的 `result` 值。

## 参数

输出路径必须以 `.csv`、`.tsv`、`.json`、`.jsonl` 或 `.xlsx` 结尾，扩展名决定格式。

## 输出

该能力写入一个本地 artifact，并返回格式、路径和字节数。JSON 保留原输入；
JSONL 每行写入一个对象；表格格式把对象键规范为稳定的字母顺序列。

## 示例

```bash
linxira-bio export table result.json result.csv --json
linxira-bio export table result.json result.xlsx --json
```

## 结果解读

CSV 用于通用交换，TSV 用于命令行生信工具，JSON 用于 SDK，JSONL 用于记录流，
XLSX 用于电子表格用户。

## 注意事项

JSONL 仅接受一个对象或对象数组。嵌套数组和对象在表格单元格中编码为 JSON
文本。XLSX 包含表头时最多支持
1,048,576 行和 16,384 列。VCF、BED 等领域文件在语义重要时必须保留原格式。

## 运行时依赖

导出器内置于 Rust 应用，不依赖外部运行时或网络连接。

## 引用

CSV 使用兼容 RFC 4180 的引用规则，XLSX 使用 ECMA-376 Office Open XML 工作簿格式。

## 故障排除

不支持的扩展名会被拒绝。混合标量数组既不是表格也不是 JSONL 记录；导出前应
转换为对象或二维数组。
