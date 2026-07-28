# MEME Motif 发现

## 用途

对核酸或蛋白 FASTA 在本地执行从头 motif 发现。

## 输入

输入本地核酸或蛋白 FASTA。

## 参数

选择字母表、出现模型、motif 数量、宽度范围和 CPU 线程。

## 输出

将 MEME 的标准 `meme.txt` 复制到指定输出，并记录受控命令元数据。

```bash
linxira-bio motif meme sequences.fa motifs.meme --alphabet dna --motifs 3 --json
```

## 示例

上面的命令使用默认宽度范围发现最多三个 DNA motif。

## 结果解读

结合 motif E-value、位点数、宽度和序列组成解读。

## 注意事项

显著性取决于背景组成、序列选择和出现模型。本包装层不下载数据库，也不再分发 MEME Suite。

## 运行时依赖

需要 `PATH` 中的 `meme`，或设置 `LINXIRA_BIO_MEME`。

## 引用

引用 MEME Suite、版本、字母表、出现模型和序列来源。

## 故障排除

审计 `meme`；仅在程序不在 `PATH` 时设置 `LINXIRA_BIO_MEME`。
