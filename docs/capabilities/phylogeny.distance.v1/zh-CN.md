# 系统发育距离矩阵

## 用途

从 FASTA 格式的多序列比对（MSA）计算成对距离矩阵。

## 输入

一个 FASTA 多序列比对文件，所有序列等长，至少包含两条序列。

## 参数

必须提供 `output`；可选 `model` 选择距离模型（`p-distance`、`jc69` 或 `k80`），默认为 `p-distance`。

## 输出

写出包含 `seq_a`、`seq_b` 和 `distance` 列的 TSV 文件（完整 N×N 矩阵）。返回序列数、比对长度、比较位点数、模型名称、距离条目和警告。

## 示例

```bash
linxira-bio phylogeny distance alignment.fa distances.tsv --model p-distance --json
linxira-bio phylogeny distance alignment.fa distances.tsv --model jc69 --json
linxira-bio phylogeny distance alignment.fa distances.tsv --model k80 --json
```

## 结果解读

- `p-distance`：差异位点比例。双空位位置从分母中排除。空位与字符比较视为差异。
- `jc69`：Jukes-Cantor 校正：d = -3/4 × ln(1 - 4/3 × p)。当 p ≥ 0.75 时产生 `Infinity`。
- `k80`：Kimura 双参数校正，利用转换/颠换比。当校正公式饱和时产生 `Infinity`。

## 注意事项

所有序列必须预先比对且等长。此功能不自行执行比对——请使用 `msa.muscle.v1` 进行比对。

## 运行时依赖

纯本地 Rust；无需外部工具或网络服务。

## 引用

Jukes TH, Cantor CR (1969). Evolution of protein molecules. *Mammalian Protein Metabolism*.

Kimura M (1980). A simple method for estimating evolutionary rates of base substitutions. *Journal of Molecular Evolution*.

## 故障排除

确保所有序列长度相同（比对格式）。`--model` 参数使用以下之一：`p-distance`、`jc69`、`k80`。输出中的 Infinity 值表示距离校正已饱和。