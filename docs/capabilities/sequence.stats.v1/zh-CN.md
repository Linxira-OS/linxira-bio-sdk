# FASTA 序列统计

## 用途

在本地统计 FASTA 记录数、长度、N50/L50、auN、GC 比例和 N 含量。

## 输入

一个可读取的 FASTA 文件。支持多行序列，标题行必须以 `>` 开头。

## 参数

- `<input.fasta[.gz]>`（必需）：FASTA 文件，支持 gzip（按魔数识别）。
- `--backend auto|rust|python|r`：实现后端。`rust`（及缺省）在进程内运行原生
  引擎；`python` 和 `r` 经 worker 路由到
  `org.linxira.benchmark-python` / `org.linxira.benchmark-r` 包
  （Biopython / Biostrings 实现），返回 V2 结果封装。`auto` 查询
  `runtime-preferences.json`：未命中则用 `rust`，命中非 rust 后端时输出
  `backend_from_preferences` 警告。
- `--json`：输出完整结果封装而非字段列表。

## 输出

返回 `sequence_count`、`total_bases`、最小/最大/平均长度、`n50`、`l50`、
`au_n`、`gc_percent`、`n_count` 和 `n_percent`。

## 示例

```bash
linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json
```

## 结果解读

N50 是达到总长度一半时的序列长度；L50 是达到该阈值所需的序列条数。两者是连续性描述，不代表组装正确性。

## 注意事项

GC 百分比的分母只包含 A/C/G/T；N 百分比使用全部序列字符。统计不会校正污染、倍性或组装错误。

## 运行时依赖

- `rust`（默认）：纯 Rust 本地能力，无需外部工具。
- `python` / `r`：对应的 benchmark 包及其解释器与依赖——Python 需
  `biopython`，R 需 `Biostrings`、`jsonlite`、`digest`。两者均无网络访问。

## 引用

N50/L50 使用常规定义；auN 为长度加权平均 `sum(length^2) / sum(length)`。

## 故障排除

- 若提示序列出现在首个标题之前，请检查文件是否确为 FASTA，并移除开头的非标题内容。
- `dependency Biostrings ... is not installed`：R 包从主机 R 环境解析依赖；
  请安装该包，或将 `LINXIRA_BIO_WORKFLOW_R_LIBRARY` 指向含它的项目库。
- `unknown --backend value`：该参数只接受 `auto`、`rust`、`python`、`r`。
