# 代谢组学峰检测

## 用途

在本地解析 mzML 质谱文件，解码 m/z 与强度数组（base64、32/64 位浮点、可选 zlib 压缩），并通过局部极大值峰拾取检测质心峰，供研究用途的代谢组学分析使用。

## 输入

包含一个或多个含 m/z 与强度二进制数组谱图的 mzML 文件（支持 gzip 压缩）。

## 参数

无需参数。

## 输出

TSV 峰表，列为 `spectrum_index`、`retention_time_min`、`mz`、`intensity`。JSON 输出报告 `spectrum_count`、`ms1_count`、`ms2_count`、`peak_count` 与完整 `peak_table`。

## 示例

```bash
linxira-bio medical metabolomics sample.mzML peaks.tsv --json
```

## 结果解读

每个峰为强度数组中的局部极大值（强度为正且大于两侧邻居），记录其 m/z 与保留时间。峰是特征分组的质心候选；MS 级别取自谱图 CV 项（MS:1000511）。

## 注意事项

仅供研究使用。峰拾取为简单的无阈值局部极大值检测器；不进行同位素去卷积、特征对齐或定量。仅解码 m/z（MS:1000514）与强度（MS:1000515）数组，其他数组类型忽略。

## 运行时依赖

无 — 纯本地 Rust 能力（内置 gzip 与 zlib 支持）。

## 引用

mzML 1.1.0 格式规范（HUPO-PSI 质谱标准）。

## 故障排除

若解析失败，请确认文件为有效 mzML XML 且包含 `<binary>` 数组、base64 数据完整，并确认数组声明为 32 位（MS:1000523）或 64 位（MS:1000521）浮点。gzip 压缩输入会自动识别。
