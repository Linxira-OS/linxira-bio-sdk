# Benchmark 跑分

## 用途

在本机将同一能力经所有已注册后端重复执行，以可披露的方法学（预热一次、计时重复、
中位数/最小/最大/四分位距、Linux 下使用 `/usr/bin/time -v`）测量耗时与峰值内存，
并验证各后端返回数值一致的结果。报告用于驱动 `runtime-preferences.json`、
服务器跑分矩阵与公开的 benchmark 页面。

## 输入

- 能力 id（例如 `sequence.stats.v1`）。
- 一个或多个输入：可按角色绑定（`fasta=INPUT.fasta`），也可直接给出裸路径
  并按能力角色顺序自动绑定。
- 可选的能力参数，以 JSON 对象形式通过 `--parameters` 传入。

## 参数

- `--backends rust,python,r`：要执行的后端（默认 `rust`）。未注册 benchmark
  pack 的后端会给出警告并跳过。
- `--repeat N`：预热一次之后的计时重复次数（默认 3）。
- `--output DIR`：报告输出目录（默认 `benchmark-results`）。
- `--dataset-class CLASS`：数据类别标签，如 `srr`、`annotation`、`matrix`、
  `structure`、`sequence`、`sample-table`、`other`。
- `--parameters JSON`：透传给 worker 的能力参数。
- `--json`：在标准结果信封内输出报告。

## 输出

- `DIR/<capability>.benchmark.json`：benchmark 报告（符合
  `benchmark-report.schema.json`），包含各后端的逐次运行、中位数、最小/最大、
  四分位距、峰值 RSS、一致性判定、字段级差异清单，以及完整环境披露
  （操作系统、内核、CPU、内存、引擎/Python/R 版本、容器标记、页缓存与
  计时精度标签）。
- `DIR/<capability>.benchmark.md`：同一报告的人类可读摘要表。
- 使用 `--json` 时，stdout 只有一个结果信封。

## 示例

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=tests/fixtures/sequences/tiny.fa --backends rust --repeat 5 --output benchmark-results --dataset-class sequence --json
```

```bash
linxira-bio benchmark run interval.intersect.v1 left-bed=a.bed right-bed=b.bed --backends rust --repeat 3 --output benchmark-results --dataset-class annotation
```

## 结果解读

- `median_wall_ms` 是核心数字；第一次（预热）运行不计时，因此页缓存为热。
- `iqr_wall_ms` 相对中位数过大说明机器不安静，应重跑后再采信。
- `precision: high` 表示每次计时运行都由 `/usr/bin/time -v` 包裹
  （wall、CPU、峰值 RSS、磁盘 IO）；`precision: degraded` 表示使用了进程内
  时钟（目前是 Windows）且没有 RSS。两者绝不可混在同一对比中。
- `consistency: consistent` 要求所有数值字段在 1e-6 相对容差内一致、
  字符串完全相等；`findings` 列出不一致的确切字段路径。
- 在该能力存在原生（Python/R）benchmark pack 之前，`speedup` 与
  `memory_saving` 保持为 `null`。

## 注意事项

- benchmark 只度量“单一能力 + 单一输入 + 单一机器”，不能跨数据类别外推。
- `benchmark.run.v1` 通过 worker 子进程执行并继承其退出契约：单次失败会被
  记录在报告中，不会中断整批。
- 不要对比不同机器或不同工具版本采集的报告；环境披露字段就是为防止这一点。

## 运行时依赖

- `linxira-bio-worker` 二进制（查找顺序：`LINXIRA_BIO_WORKER`、CLI 可执行文件
  同目录、`PATH`）。
- Linux 上的 `/usr/bin/time`（GNU time），用于高精度指标；可选。
- 无网络访问。

## 引用

- 方法学、一致性容差与披露要求：仓库 `ROADMAP.md` §7（M2）。
- 报告 schema：`schemas/benchmark-report.schema.json`。

## 故障排除

- `the worker binary linxira-bio-worker was not found`：安装 SDK，或将
  `LINXIRA_BIO_WORKER` 指向二进制路径。
- `skipping backend "python": no benchmark pack is registered yet (M2-T3)`：
  该能力的原生 pack 尚不可用，实际只运行了 `rust`。
- `consistency: inconsistent`：检查 `findings` 中不一致的字段路径，并确认两个
  后端读取的是同一份绝对路径输入。
- 某次运行指标为空且失败：查看运行记录中被截断的 stderr 摘要；常见原因是
  输入文件不存在或缺少原生工具。
