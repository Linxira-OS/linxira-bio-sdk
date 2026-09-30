# 比对去重标记

## 用途

用原生 samtools 链为 BAM 比对结果标记 PCR 与光学重复，为感知重复的变异检测做准备。

## 输入

BAM 比对文件（不要求坐标排序，链内会重新排序）。需要双端数据——fixmate 元数据是
markdup 的输入。

## 参数

`--threads` 设置 samtools 工作线程（默认 1）。`--stats` 输出重复统计并保留带重复标记的
读段。输入与输出路径必须不同。

## 输出

写出重复记录带 0x400 标志的 BAM。JSON 报告工具 `samtools`、模式 `markdup`、输出路径与
字节数、线程数、`command_count` 为 4、以及警告。中间文件暂存于临时目录并随后删除。

## 示例

```bash
linxira-bio alignment markdup aligned.bam marked.bam --stats --json
```

## 结果解读

重复只标记、绝不删除；由下游工具决定如何使用该标志。内部四步为 collate、fixmate -m、
sort、markdup——正是 samtools markdup 文档要求的前处理。

## 注意事项

单端文库的 fixmate 元数据无意义、标记不可靠。需要 PATH 上的 `samtools` 或经
`LINXIRA_BIO_SAMTOOLS` 指定；该二进制仅被调用，绝不捆绑分发。

## 运行时依赖

原生 `samtools` 可执行文件（环境审计在 alignment-files 类别下探测）。不使用 Python、R
或 Java 运行时。

## 引用

报告去重结果时引用 samtools 套件：Li 等 2009（Bioinformatics 25:2078-2079）及 samtools
手册中的 markdup 文档。

## 故障排除

先运行环境审计。确认 BAM 为双端数据；samtools 对单端输入发出的 fixmate 警告表明结果
不应被信任。
