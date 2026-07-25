# BED 区间扣除

## 用途

从左侧 BED 区间中扣除右侧 BED 区间覆盖的碱基，并把剩余片段写成确定性的 BED3 文件。

## 输入

两个可读取的 BED 文件：左侧为需要保留的区间，右侧为需要扣除的区间。程序按魔数识别
纯文本和 gzip 流；每条记录至少需要三个制表符分隔列。

## 参数

必须提供 `<left.bed>`、`<right.bed>` 和 `<output.bed>`，输入顺序有意义。`--json` 返回
标准结果封装。

## 输出

写出新的 BED3 文件，包含保留下来的左侧片段。JSON 返回左/右区间数、输出区间数、受影响
左侧区间数、被扣除碱基数、输出碱基数、各 contig 统计和警告。

## 示例

```bash
linxira-bio interval subtract genes.bed repeats.bed genes-without-repeats.bed --json
```

## 结果解读

区间按 `[start, end)` 处理。右侧区间只扣除与左侧重叠的碱基；单个左侧区间可能被切分成
多个输出片段。

## 注意事项

v1 只输出 BED3；不会保留 name、score、strand 或额外 BED 列，也不支持链特异、重叠比例或
互惠重叠规则。

## 运行时依赖

纯 Rust 本地能力，无需 Python、R、Java、bedtools 或外部命令行工具。

## 引用

坐标语义遵循 UCSC BED 规范。扣除行为采用标准区间代数，与 bedtools subtract 的非链特异
基础扣除语义相近。

## 故障排除

输出为空时先确认左右文件使用相同参考基因组和 contig 命名。如果命令拒绝覆盖输出，请
选择新的输出路径，或在确认后手动删除旧文件。
