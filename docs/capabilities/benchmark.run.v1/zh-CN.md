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

- `--backends rust,python,r`：要执行的后端（默认 `rust`）。`rust` 是原生引擎；
  `python` 和 `r` 会把请求路由到对应的 benchmark pack
  （`org.linxira.benchmark-python`、`org.linxira.benchmark-r`），pack 必须内置该
  能力的实现；pack 未实现该能力时该后端报错退出，其余后端继续。
  `--backends auto` 会被拒绝：跑分不得读取 `runtime-preferences.json`——
  偏好表本身就是跑分推导出来的。
- `--repeat N`：预热一次之后的计时重复次数（默认 3）。
- `--output DIR`：报告输出目录（默认 `benchmark-results`）。
- `--dataset-class CLASS`：数据类别标签，如 `srr`、`annotation`、`matrix`、
  `structure`、`sequence`、`sample-table`、`other`。
- `--parameters JSON`：透传给 worker 的能力参数。python/r 每次运行会写入一个
  全新的临时输出目录，运行结束即删除；原生引擎不需要。
- `--json`：在标准结果信封内输出报告。

## 输出

- `DIR/<capability>.benchmark.json`：benchmark 报告（符合
  `benchmark-report.schema.json`），包含各后端的逐次运行、中位数、最小/最大、
  四分位距、峰值 RSS、一致性判定、字段级差异清单，以及完整环境披露
  （操作系统、内核、发行版、CPU、内存、引擎/Python/R 版本、容器标记、页缓存与
  计时精度标签）。在 WSL 内运行时，披露还会额外记录 Windows 主机信息：`cmd.exe
  /c ver` 的版本串、机型、CPU 型号、逻辑处理器数与物理内存，均通过 interop
  桥探测；interop 不可用时这些字段直接省略而非猜测。每条运行记录带可选的
  `self_reported_wall_ms` 与 `self_reported_peak_rss_mb`（来自 pack 的进程内
  埋点），可以看清 `wall_ms` 中解释器启动占比。
- `DIR/<capability>.benchmark.md`：同一报告的人类可读摘要，含自报计时表。
- `speedup` = `median_wall(首个非 rust 后端) / median_wall(rust)`；
  `memory_saving` = `1 - median_peak_rss(rust) / median_peak_rss(首个非 rust
  后端)`；任一侧无结果则保持 `null`。markdown 摘要会写明对比的是哪个后端。
- 使用 `--json` 时，stdout 只有一个结果信封。

## 示例

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=tests/fixtures/sequences/tiny.fa --backends rust --repeat 5 --output benchmark-results --dataset-class sequence --json
```

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=input.fa --backends rust,python,r --repeat 5 --output benchmark-results --dataset-class sequence
```

## 结果解读

- `median_wall_ms` 是核心数字；第一次（预热）运行不计时，因此页缓存为热。
- `iqr_wall_ms` 相对中位数过大说明机器不安静，应重跑后再采信。
- `precision: high` 表示每次计时运行都由 `/usr/bin/time -v` 包裹
  （wall、CPU、峰值 RSS、磁盘 IO）；`precision: degraded` 表示使用了进程内
  时钟（目前是 Windows）且没有 RSS。两者绝不可混在同一对比中。
- `consistency: consistent` 要求所有后端的 `result` 数值字段在 1e-6 相对容差内
  一致、字符串完全相等；`findings` 列出不一致的确切字段路径。信封的
  provenance（时间戳、版本、单次运行的 artifacts）有意排除在对比之外——
  只评判分析结果本身。
- `speedup` 对比摘要中写明的原生后端与 `rust`；存在多个原生后端时，请阅读各
  后端行获取完整信息（runtime 偏好表会保存每个后端的数字）。

## 注意事项

- benchmark 只度量“单一能力 + 单一输入 + 单一机器”，不能跨数据类别外推。
- `benchmark.run.v1` 通过 worker 子进程执行并继承其退出契约：单次失败会被
  记录在报告中，不会中断整批。pack 拒绝请求（manifest 哈希过期、能力未实现、
  输入不可读）时，报告判为 `failed` 并给出指名后端的 finding。
- python/r 后端的 `wall_ms` 包含解释器启动与 pack 校验；自报列用于分离纯分析
  耗时。worker 会在 pack 运行前后各对输入做一次哈希，因此超大输入会额外付出
  哈希成本——如实披露、不隐藏，且各次重复之间一致。
- 不要对比不同机器或不同工具版本采集的报告；环境披露字段就是为防止这一点。
  pack 库版本偏离 lock 时会以 warning 诊断标出，并写入信封的 provenance。

## 运行时依赖

- `linxira-bio-worker` 二进制（查找顺序：`LINXIRA_BIO_WORKER`、CLI 可执行文件
  同目录、`PATH`）。
- Linux 上的 `/usr/bin/time`（GNU time），用于高精度指标；可选。
- `--backends python`：`org.linxira.benchmark-python` pack（worker V2 契约）、
  Python 3 解释器，以及 pack 锁定的依赖（`biopython`）；实际使用的版本会被
  记录。
- `--backends r`：`org.linxira.benchmark-r` pack、R 环境，以及 `jsonlite`、
  `digest` 和该能力所需的 R 包（`sequence.stats.v1` 需要 `Biostrings`）。
  设置了 `LINXIRA_BIO_WORKFLOW_R_LIBRARY` 时优先使用；实际解析到的库版本会
  披露。与其它平台混用的项目库（例如与 WSL 共享的 Windows 编译
  `.linxira-bio/ci` 目录）不可被自动发现：请把 workflow 根目录
  （`LINXIRA_BIO_WORKFLOW_ROOT`）移出共享检出，或去掉该库。
- 无网络访问。

## 引用

- 方法学、一致性容差与披露要求：仓库 `ROADMAP.md` §7（M2）。
- 报告 schema：`schemas/benchmark-report.schema.json`。
- benchmark pack：`workflows/org.linxira.benchmark-python/README.md`、
  `workflows/org.linxira.benchmark-r/README.md`。

## 故障排除

- `the worker binary linxira-bio-worker was not found`：安装 SDK，或将
  `LINXIRA_BIO_WORKER` 指向二进制路径。
- `benchmark pack org.linxira.benchmark-python has no python implementation
  of <capability>`：pack 只承载 README 中列出的能力；其余后端照常运行。
- `workflow file verification failed: <path>`：pack 的 `manifest.json` 哈希与
  文件不一致；运行 `python scripts/update-pack-manifest.py workflows/<pack-id>`
  并提交。
- `workflow output parent is not a directory` 或 `refusing to overwrite
  workflow output directory`：临时输出目录冲突或被中断的运行遗留；重跑即可。
- `consistency: inconsistent`：检查 `findings` 中不一致的字段路径，并确认所有
  后端读取的是同一份绝对路径输入。
- 某次运行指标为空且失败：查看运行记录中被截断的 stderr 摘要；常见原因是输入
  文件不存在、缺少原生工具（例如 R pack 缺 `Biostrings`），或库是为其它平台
  编译的。
