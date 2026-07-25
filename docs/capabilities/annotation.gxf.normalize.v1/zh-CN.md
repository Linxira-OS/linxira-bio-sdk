# annotation.gxf.normalize.v1

## 用途

验证 GFF3 或 GTF，并输出规范 GFF3。

## 输入

一个普通文本或 gzip 压缩的 GFF3/GTF 注释。

## 参数

- `--sort`：按序列、起点、终点、特征类型和来源排序。
- `--json`：输出结构化摘要。

## 输出

新的 GFF3 文件，以及输入/输出记录数、GTF 属性转换记录数、排序状态和警告。

## 示例

```bash
linxira-bio annotation normalize input.gtf output.gff3 --sort --json
```

## 结果解读

通过转换记录数确认是否检测到 GTF 风格属性；GFF3 保留分隔符会进行百分号编码。

## 注意事项

该命令只验证并规范语法，不推断或修复缺失的生物学关系，也不会覆盖已有输出。

## 运行时依赖

在 Rust core 中本地运行，不需要外部运行时或网络。

## 引用

未使用外部算法；输出遵循仓库确定性的 GFF3 序列化约定。

## 故障排除

转换失败时，按报告的行号检查列数、坐标、链方向、phase 和属性。
