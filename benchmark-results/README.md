# Benchmark 入库规范（Archival Convention）

> 本目录是所有 benchmark 结果的**唯一归档入口**。每次跑分入库必须遵守本规范；
> `scripts/validate-repository.py` 会对 `*/summary.json` 做机器校验（schema + 报告编号
> 唯一性 + 溯源链完整性），不合规的入库会让 CI 直接失败。
> This directory is the single archival home for benchmark results; every run
> must follow this convention and is machine-checked in CI.

## 1. 目录与编号（Layout & Report IDs）

```
benchmark-results/
  <YYYY-MM-DD>/                 # 一次入库一个日期目录（同日多次跑分可同目录追加或建多目录）
    summary.json                # 必有：入库单元，见 §2
    summary.md                  # 必有：人类可读版（含完整环境披露）
    <capability>.benchmark.json # 每个能力一份完整报告（CLI 原样输出）
    <capability>.benchmark.md
```

- 报告编号：`bench-YYYYMMDD-NNN`（如 `bench-20260913-001`）。`NNN` 从 `001`
  开始；入库前扫描本目录已有 summary.json 的 `report_id`，同日取最大号 +1，
  **编号绝不复用**（校验器会拒绝重复编号）。
- 编号的日期段必须与所在目录日期一致（校验器强制）。

## 2. summary.json 必填字段（Mandatory Fields）

schema：`schemas/benchmark-summary.schema.json`。逐字段：

| 字段 | 必填 | 说明 |
|---|---|---|
| `report_id` | ✅ | 上节编号规则 |
| `purpose` | ✅ | **一句话写清这次测的是什么、为什么测**（如"三端一致性基线""VCF 大文件吞吐"） |
| `methodology` | ✅ | backends / repeat / 预热 / 页缓存 / 口径声明（warm 与 cold 不得混排） |
| `environment` | ✅ | 完整环境披露（与 benchmark-report 同构）。WSL 内运行必须含 Windows 宿主机字段（host_os/host_model/host_cpu_model/host_logical_processors/host_total_memory_mb）与 wsl_distro/wsl_version |
| `data_sources[]` | ✅ | **每个输入数据集一条**：`id`、`origin`（repository-fixture / public-download / private-server / generated）、`reference`（仓库路径或来源 URL）、`note`（SRR/PRJ/DOI 检索号等） |
| `entries[]` | ✅ | 每能力一条：测了什么（`dataset` + 参数）、`data_source_id` 必须指向 data_sources 里的 id（溯源链，校验器强制）、结果指标、`report` 指向完整报告文件 |

## 3. 原始数据政策（Raw Data Policy）

- **仓库 fixture**（`tests/fixtures/`）：本体已在 git 里，`origin` 标
  `repository-fixture`，`reference` 写路径即可。
- **公开下载**（SRA/ENCODE/论文补充材料等）：原始数据**永不入 git**；
  `origin` 标 `public-download`，`reference` 写 URL + `note` 写检索号，
  输入完整性靠各报告 `provenance.input_sha256` 追溯（见
  `docs/BENCHMARK_DATA_SOURCES.md`）。
- **私服数据**：等服务器接入规范下发后按规范执行（默认**只回传指标，不动原始
  数据，不删除任何东西**），`origin` 标 `private-server`。
- 入库物 = 指标 + 报告 + 环境，**永远不含原始数据本体**。

## 3a. 版本溯源（Code Revision，强制）

- **每次评估必须标注代码版本**：summary 与每份报告的 environment 均携带
  `code_revision`（构建时嵌入的 git sha）。同一数据在不同开发版本上测出的
  数字**分库存放、禁止混排对比**；对比表必须写明各自的 `report_id` 与
  `code_revision`。
- 引擎自 1.0.2 起自动嵌入 sha；1.0.1 及更早的历史报告回填时在 summary.json
  的 environment 里手工补 `code_revision`（如 bench-20260913-001 → 9052bcf）。

## 3b. 数据保留（Retention，强制）

- **重测不覆盖**：同一数据可以反复重测（包括对方机器已算过的），但每次测量
  都是新的一次入库——新 `report_id`、新日期目录或同目录新编号，**旧结果永不
  覆盖、永不删除**；版本间对比靠 report_id + code_revision 关联。
- **未跑完也保留**：中断/部分完成的批次以 `status: "partial"` 入库并注明
  缺口（哪些 run 未完成、原因），不得因"没跑完"而丢弃已测得的数据。

## 4. 环境入库口径（Environment Disclosure）

每份报告与 summary 都携带完整环境（os/kernel/distro/guest cpu/memory、
engine/Python/R 版本、容器标记、页缓存、计时精度）。跨机器、跨 WSL/裸机、
warm/cold 的数字**禁止混排对比**；对比表必须注明来源 `report_id`。

## 5. 实现版本共存与迭代日志（Versioned Implementations & Iteration Log）

- 同一分析方法的多个实现版本（v1/v2）**可以同时存在**：能力 id 本身带版本后缀
  （`foo.v1`、`foo.v2` 是两个独立能力，契约、测试、pack 各自独立），旧版本在
  明确废弃前保持可用，benchmark 可同时测两个版本对比。
- **每次算法/实现迭代必须记日志**：`docs/IMPLEMENTATION_LOG.md`，按能力一行
  （日期、改了什么、为什么、证据 `report_id` / 测试）。改代码不记日志视为流程
  违规；重写算法（v2）必须附新旧版本对比的 benchmark 报告编号。

## 6. 入库 Checklist（提交前自检）

1. `summary.json` 通过 `schemas/benchmark-summary.schema.json`；
2. `report_id` 唯一且与目录日期一致；
3. 每个 entry 的 `data_source_id` 都能指到 `data_sources` 里的 id；
4. `summary.md` 含完整环境披露（WSL 运行含 Windows 宿主机段）；
5. 原始数据未入库（fixtures 除外）；
6. 算法有变更的，`docs/IMPLEMENTATION_LOG.md` 已更新；
7. `python scripts/validate-repository.py` 全绿。

## 7. 现有归档（Index）

| report_id | 日期 | 内容 |
|---|---|---|
| `bench-20260913-001` | 2026-09-13 | 4 能力三端（rust/python/r）一致性 + 速度基线（仓库 fixture，WSL2 Arch） |
