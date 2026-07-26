# 基因组特征密度

## 用途

从本地 GFF3 或 GTF 计算滑动窗口注释特征数和每百万碱基密度。

## 输入

一个普通或 gzip 的 GFF3/GTF 注释文件。

## 参数

`feature_types` 默认是 `gene`；`window_size` 和 `step_size` 默认均为 1,000,000 碱基，且必须为正数。

## 输出

返回选中特征数，以及每条序列各窗口的坐标、计数、密度和警告。

## 示例

```bash
linxira-bio annotation gene-density genes.gff3 --window-size 1000000 --step-size 250000 --json
```

## 结果解读

一个特征会计入所有与其 1 起始闭区间坐标相交的窗口。

## 注意事项

序列长度由最大注释终点推断，因此无法知道末端没有注释的空白区域。

## 运行时依赖

纯本地 Rust；不需要参考 FASTA、Python、R、Java、网络或外部程序。

## 引用

每个裁剪窗口的密度为 `feature_count * 1,000,000 / window_width`。

## 故障排除

使用准确的注释特征名称，并选择能让结果少于 2,000,000 个窗口的窗口和步长。
