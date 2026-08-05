# 系统发育树可视化

## 用途

将 Newick 格式的系统发育树渲染为矩形分支图 SVG 图像。

## 输入

Newick 格式的树文件（纯文本或 gzip 压缩），至少包含 2 个叶节点。

## 参数

`--width` 和 `--height` 控制输出图像尺寸（200–4096，默认 800×600）。
`--font-size` 设置叶节点标签字体大小（6–48，默认 14）。
`--no-branch-lengths` 绘制均匀分支图，不按分支长度缩放。

## 输出

带标签叶节点和分支线的 SVG 图像。JSON 结果包含可视化元数据（叶节点数量和尺寸）。

## 示例

```bash
linxira-bio phylogeny tree-plot tree.nwk tree.svg --json
linxira-bio phylogeny tree-plot tree.nwk tree.svg --width 1200 --height 800 --no-branch-lengths --json
```

## 结果解读

树从左到右绘制，叶节点标签在右侧。分支存在时按比例缩放。内部节点绘制为连接线。
可视化结果为分支图（非带比例尺的谱系图）。

## 注意事项

树必须至少包含 2 个叶节点，最多 1,000,000 个节点。Newick 文件解压后不得超过 128 MiB。
仅支持矩形样式。

## 运行时依赖

纯 Rust 实现，无需外部工具。

## 引用

渲染算法无需外部引用。

## 故障排查

验证输入文件是否为有效的 Newick 格式。如果树无法渲染，请检查每个叶节点是否有标签，
以及文件是否以分号结尾。