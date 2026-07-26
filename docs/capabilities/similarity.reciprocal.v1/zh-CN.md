# 双向最佳命中

## 用途

从已经完成的正向和反向 BLAST 结果中寻找确定性的双向最佳命中对。

## 输入

两个普通或 gzip BLAST 结果文件，角色分别为 `forward` 和 `reverse`。

## 参数

可选 `max_evalue` 必须非负；可选 `min_identity_percent` 必须在 0 到 100 之间。

## 输出

返回查询数、双向配对、未配对数、两个方向的得分、相似度和警告。

## 示例

```bash
linxira-bio similarity rbh forward.tsv reverse.tsv --max-evalue 1e-5 --min-identity 30 --json
```

## 结果解读

按 e-value、bit score、相似度、比对长度和目标标识符依次确定最佳命中。

## 注意事项

双向最佳命中只是直系同源候选，不是对一对一直系同源或功能守恒的证明。

## 运行时依赖

纯本地 Rust；两个方向的搜索都必须已经完成。

## 引用

该方法实现双向最佳命中启发式，并明确规定并列结果顺序。

## 故障排除

确认正向目标标识符与反向查询标识符能够对应，反方向亦然。
