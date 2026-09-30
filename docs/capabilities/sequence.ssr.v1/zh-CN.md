# 简单序列重复挖掘

## 用途

在 FASTA 序列中检测完美简单序列重复（SSR / 微卫星，单碱基至六碱基基序），用于标记开发
与基因组重复普查；语义对齐 MISA，后端可在 rust / python / r 间选择。

## 输入

单个 FASTA 文件（可 gzip）。标识符取每条 header 的第一个空白分隔 token。

## 参数

`--min-repeats` 接受 `长度:次数` 列表（默认 `1:10,2:6,3:5,4:5,5:5,6:5`，即 MISA 标准；
未指定的长度沿用默认值）。`--compound-distance` 设定复合 SSR 分组的最大中断距离
（碱基数，默认 100，即 MISA 的 interruption_max_distance）。`--backend
auto|rust|python|r` 选择实现：原生 Rust 扫描、经 python benchmark pack 的 pytrf C 内核、
或经 r benchmark pack 的 Biostrings。`auto` 查询 runtime-preferences.json。

## 输出

每个重复一行的 TSV（sequence_id、motif_length、motif、repeats、size、start、end、
compound）与 JSON 汇总：序列数、SSR 数、总碱基数、复合组数、按基序长度的计数、逐序列
统计。

## 示例

```bash
linxira-bio sequence ssr genome.fa ssr.tsv --min-repeats 2:8 --compound-distance 100 --json
linxira-bio benchmark run sequence.ssr.v1 --fasta genome.fa   # 三后端对比
```

## 结果解读

每个位置报告满足阈值的最短基序串联重复并消费整个区间，记录互不重叠——与 MISA 一致。
基序按其在序列中的实际形态报告。跨序列比较请用 SSR 密度（每 kb 计数）而非原始计数。
复合组（中断距离内的相邻记录）近似 cSSR 计数。

## 注意事项

只检测完美重复；中断重复仅通过复合分组体现。ACGT 之外的碱基（N）打断重复、绝不进入
基序。python/r 后端需要各自的锁定 pack 环境；后端 diff 不一致是 bug 而非噪声。

## 运行时依赖

Rust 后端：无。Python 后端：benchmark pack 锁定的 pytrf 环境。R 后端：benchmark pack
锁定的 Biostrings。

## 引用

报告 SSR 普查时引用 Thiel 等 2003（Theor. Appl. Genet. 106:1231-1238，MISA）的语义与
阈值；与 TRF 来源重复对比时引用 Benson 1999（Nucleic Acids Res. 27:573-580）。

## 故障排除

FASTA 非空而输出为空，通常是阈值严于数据（放宽 `--min-repeats` 次数或检查基序长度
1-6）。后端路由失败会提示缺失的 pack 环境；运行环境审计安装锁定的 python/r 依赖集。
