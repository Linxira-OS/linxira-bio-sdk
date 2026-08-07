# RNA 二级结构预测

## 用途

使用 ViennaRNA RNAfold 工具预测 RNA 序列的最小自由能（MFE）二级结构。

## 输入

包含一条或多条 RNA 序列的 FASTA 文件。每条序列应使用标准 RNA 核苷酸字符（A、U、G、C）。

## 参数

`--temp` 设置折叠温度（摄氏度，0–100，默认 37.0）。

## 输出

包含预测二级结构（点括号表示法）和最小自由能值的文本文件。JSON 结果包含结构元数据。

## 示例

```bash
linxira-bio rna secondary-structure input.fa output.txt --json
linxira-bio rna secondary-structure input.fa output.txt --temp 25.0 --json
```

## 结果解读

输出使用点括号表示法：点（`.`）表示未配对碱基，匹配的括号 `()` 表示配对碱基。最小自由能（kcal/mol）表示预测结构的热力学稳定性；越负的值表示结构越稳定。

## 注意事项

需要在系统上安装 ViennaRNA RNAfold。预测基于热力学模型，可能不代表生物活性构象。仅支持单序列折叠，不预测假结。

## 运行时依赖

需要 ViennaRNA RNAfold（2.x 版本）。可通过系统包管理器安装或从 ViennaRNA 网站下载。

## 引用

Lorenz, R., et al. (2011). ViennaRNA Package 2.0. Algorithms for Molecular Biology, 6:26.

## 故障排除

如果找不到 RNAfold，请通过系统包管理器安装 ViennaRNA。验证输入 FASTA 文件包含有效的 RNA 序列，仅使用 A、U、G、C 字符。温度值超出 0–100 °C 范围将被拒绝。