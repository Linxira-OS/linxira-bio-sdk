# BAM/CRAM 转 BigWig

## 用途

从 BAM 或 CRAM 比对数据在本地生成 BigWig 覆盖轨道。

## 输入

已建立索引的 BAM 或 CRAM 比对文件。

## 参数

`--threads` 控制原生工具工作线程数。

## 输出

BigWig 覆盖轨道，以及可选 JSON 结果信封。

## 示例

```text
linxira-bio alignment bam-to-bigwig reads.bam coverage.bw --json
```

## 结果解读

轨道按照已安装原生工具的默认值表示覆盖度。

## 注意事项

此初始封装不提供归一化和 binning 参数。

## 运行时依赖

本地 deepTools `bamCoverage`，可由 `LINXIRA_BIO_BAMCOVERAGE` 配置。

## 引用

应引用 deepTools 及生成输入的比对方法。

## 故障排除

确认比对文件已建立索引且 `bamCoverage` 可用。
