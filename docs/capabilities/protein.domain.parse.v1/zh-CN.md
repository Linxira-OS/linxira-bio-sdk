# 蛋白质结构域结果解析

## 用途

把已经完成的 InterProScan TSV 或 HMMER domtblout 结构域注释解析为统一的本地结构。

## 输入

一个普通或 gzip 的 InterProScan TSV 或 HMMER domtblout 文件。

## 参数

不接受分析参数。

## 输出

返回格式、序列和命中数量、来源/登录号计数、结构域坐标、注释和警告。

## 示例

```bash
linxira-bio protein domains interproscan.tsv --json
```

## 结果解读

InterProScan 第 9 列保留为 `score`；HMMER 的结构域 e-value 和得分保持原始含义。

## 注意事项

该能力只解析已经完成的搜索，不会在输入得分之外判断结构域显著性。

## 运行时依赖

纯本地 Rust；仅解析时不需要搜索软件或数据库。

## 引用

字段遵循 InterProScan TSV 与 HMMER domtblout 格式规范。

## 故障排除

保留 InterProScan 的全部制表符列，或完整的 HMMER domtblout 记录。
