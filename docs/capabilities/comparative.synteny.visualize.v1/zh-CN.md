# 共线性锚点可视化

## 用途

从含 `source_id`、`source_position`、`target_id` 和 `target_position` 的本地 TSV 渲染 SVG，支持双轨、多轨、微共线性和环形布局。

## 输入

一份制表符分隔的锚点表。

## 参数

必须提供 SVG 输出路径。

## 输出

返回归一化锚点连接 SVG。

## 示例

```text
linxira-bio comparative synteny-plot anchors.tsv synteny.svg --style circular --json
```

## 结果解读

每条曲线代表输入锚点；本渲染器不推断共线性。微共线性布局会将提供的锚点子集聚焦渲染为双轨图。

## 注意事项

最多渲染前 2,000 个锚点。

## 运行时依赖

仅使用本地 Rust。

## 引用

应引用上游锚点或共线性方法。

## 故障排除

请使用含有限数值位置的制表符分隔表。
