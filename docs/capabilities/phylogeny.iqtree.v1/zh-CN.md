# IQ-TREE 系统发育推断

## 用途

从本地多序列比对推断最大似然系统发育树。

## 输入

输入本地多序列比对。

## 参数

设置线程数、例如 `MFP` 的模型表达式，以及确定性的随机种子。

## 输出

工作文件位于隔离临时目录，最终 `.treefile` 被复制为指定的 Newick 输出。

```bash
linxira-bio phylogeny iqtree alignment.fa tree.nwk --model MFP --threads 4 --seed 1 --json
```

## 示例

上面的命令执行模型搜索和最大似然推断。

## 结果解读

结合所选模型和采样设计解读拓扑及枝长。

## 注意事项

模型选择和 bootstrap 支持是不同的科学决策；本版本只返回推断树，不声称完成 bootstrap。

## 运行时依赖

需要 `iqtree2`，或设置 `LINXIRA_BIO_IQTREE`。

## 引用

引用 IQ-TREE、版本、所选模型、随机种子和输入比对方法。

## 故障排除

审计 `iqtree`；程序不在 `PATH` 时设置 `LINXIRA_BIO_IQTREE`。
