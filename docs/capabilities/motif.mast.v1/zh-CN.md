# MAST 模体扫描

## 用途

使用 MAST 算法在核苷酸或蛋白质序列中扫描 MEME 格式模体文件中的模体出现位置。

## 输入

MEME 格式模体文件和待扫描的 FASTA 序列文件。

## 参数

`--evalue` 设置 E 值阈值（默认 1e-5）。`--hit-list` 输出精简命中列表。`--threads` 控制并行搜索线程数。

## 输出

MAST 文本输出，列出模体出现位置、得分和 E 值。JSON 结果包裹原生工具执行元数据。

## 示例

```bash
linxira-bio motif mast motifs.meme sequences.fa hits.txt --evalue 1e-5 --hit-list --threads 4 --json
```

## 结果解读

审查每个模体命中的 E 值和位置。E 值越低表示匹配越显著。验证模体出现是否与预期的生物学背景一致。

## 注意事项

模体文件必须为有效的 MEME 格式。MAST 需要安装 MEME Suite。序列数量和模体数量线性影响运行时间。

## 运行时依赖

MEME Suite（mast 可执行文件）。设置 `LINXIRA_BIO_MAST` 可覆盖二进制路径。

## 引用

Bailey TL, Gribskov M. Combining evidence using p-values: application to
sequence homology searches. Bioinformatics. 1998;14(1):48-54.

## 故障排除

验证模体文件是否为有效的 MEME 格式输出。若找不到 MAST，请安装 MEME Suite 或将
`LINXIRA_BIO_MAST` 设置为正确的二进制路径。