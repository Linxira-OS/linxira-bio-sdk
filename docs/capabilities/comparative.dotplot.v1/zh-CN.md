# 比较基因组点图

## 用途

生成基于 k-mer 的点图 SVG 可视化，比较两个 FASTA 序列以识别相似区域、重复序列和结构重排。

## 输入

两个 FASTA 文件：查询序列和参考序列。每个文件应包含至少一条序列记录。

## 参数

`--width` 和 `--height` 控制输出图像尺寸（200–4096，默认 800×800）。
`--kmer` 设置匹配的 k-mer 大小（1–32，默认 12）。

## 输出

一个 SVG 点图图像，每个匹配的 k-mer 位置绘制为一个点。JSON 结果包含可视化元数据（匹配数和尺寸）。

## 示例

```bash
linxira-bio comparative dotplot query.fa reference.fa dotplot.svg --json
linxira-bio comparative dotplot query.fa reference.fa dotplot.svg --width 1200 --height 1200 --kmer 15 --json
```

## 结果解读

每个点代表查询序列（y 轴）与参考序列（x 轴）之间的 k-mer 匹配。对角线表示相似区域。反向对角线表示倒位。对角线中的间隙表示插入或缺失。

## 注意事项

输入序列必须是有效的 FASTA 格式。非常大的基因组可能产生过于密集的图，难以解读。k-mer 大小影响灵敏度和特异性：较小的 k-mer 能找到更多匹配但可能包含噪声。

## 运行时依赖

纯 Rust 实现，无需外部工具。

## 引用

点图算法无需外部引用。

## 故障排除

如果点图为空，请尝试减小 k-mer 大小。如果图过于密集，请增大 k-mer 大小。确保输入文件为有效的 FASTA 格式，使用标准核苷酸字符。