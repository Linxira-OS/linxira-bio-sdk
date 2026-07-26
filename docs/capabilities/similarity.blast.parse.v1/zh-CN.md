# BLAST 结果解析

## 用途

把已经完成的 BLAST 表格或旧版 XML1 结果解析为确定性的本地结果。

## 输入

一个普通或 gzip 文件，支持 outfmt 6、带 `# Fields` 的 outfmt 7，以及旧版 BLAST XML1。

## 参数

不接受分析参数。

## 输出

返回格式、命中/查询/目标数量、得分摘要、标准化命中记录和警告。

## 示例

```bash
linxira-bio similarity blast-parse results.tsv --json
```

## 结果解读

坐标和得分保留来源报告语义，相似度以百分比表示。

## 注意事项

解析不会执行搜索，也不会判断数据库是否完整，更不能单独证明同源关系。

## 运行时依赖

纯本地 Rust；必须已经存在搜索结果文件。

## 引用

字段含义遵循 BLAST 表格输出和旧版 XML1 结果约定。

## 故障排除

outfmt 6 使用标准列；outfmt 7 保留 `# Fields` 声明；XML 请导出旧版 XML1 而不是 XML2。
