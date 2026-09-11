# R benchmark pack / R 基准后端包

`org.linxira.benchmark-r` hosts the **independent R implementations** of
Linxira Bio capabilities that also exist natively in the Rust engine. It is the
`r` backend of `linxira-bio benchmark run` (ROADMAP M2-T3) and the home of the R
side of the three-way implementations planned for M3.

`org.linxira.benchmark-r` 收纳 Linxira Bio 各能力的**独立 R 实现**——这些能力在 Rust 引擎中
同样有原生实现。它是 `linxira-bio benchmark run` 的 `r` 后端（路线图 M2-T3），也是 M3 三端
实现中 R 一侧的落点。

## How it is invoked / 调用方式

The worker routes a request here when `execution.backend` is `"r"`:

```text
linxira-bio benchmark run sequence.stats.v1 fasta=tests/fixtures/sequences/tiny.fa \
  --backends rust,python,r --repeat 5 --output benchmark-results
```

Standalone (worker V2 request; `--result` must be
`<parameters.output_directory>/result.json`):

```text
Rscript src/benchmark_harness.R --request request.json --result out/result.json
```

worker 在请求携带 `execution.backend = "r"` 时把整个请求交给本包；也可以脱离 worker 直接运行
入口脚本（请求为 worker V2 格式，`--result` 必须位于 `parameters.output_directory` 之下）。

## Contract / 契约

* Input roles and parameters are **exactly** those of the native capability;
  anything the Rust contract does not accept is rejected.
* The `result` object is field-for-field comparable with the Rust engine
  (`benchmark run` diffs it with a 1e-6 relative tolerance).
* One artifact, `<capability>.result.json` (kind `report`, format `json`),
  holds the result object; the envelope itself is `result.json`.
* An `info` diagnostic with code `benchmark.self_reported` carries the
  in-process wall time (`Sys.time` around the analysis only) and peak RSS
  read from `/proc/self/status` `VmHWM` (`null` where the kernel does not
  expose it). The outer `/usr/bin/time -v` measurement in the report includes
  interpreter start-up; the gap is disclosed, not hidden.
* **Library policy differs from the analysis packs**: this pack measures the
  R stack as installed on the benchmark host. `LINXIRA_BIO_WORKFLOW_R_LIBRARY`
  is honoured when set; otherwise the interpreter's own library paths are
  used. Every loaded package version is recorded in `provenance.software`,
  and a `warning` diagnostic flags versions outside `dependencies.lock.json`.

* 输入角色与参数与原生能力**完全一致**，Rust 契约不接受的内容一律拒绝。
* `result` 对象与 Rust 引擎逐字段可比（1e-6 相对容差）。
* 产物固定一个：`<capability>.result.json`（kind `report`，format `json`），envelope 本身为
  `result.json`。
* `benchmark.self_reported` 这条 `info` 诊断携带进程内墙钟时间（仅包裹分析本身）和从
  `/proc/self/status` `VmHWM` 读到的峰值 RSS（内核不提供时为 `null`）。外层 `/usr/bin/time -v`
  含解释器启动，两者差值如实披露。
* **库解析策略与分析类 pack 不同**：本包测量的是基准主机上"装了什么就用什么"的 R 栈。设置了
  `LINXIRA_BIO_WORKFLOW_R_LIBRARY` 就优先用它，否则用解释器自身的库路径；所有加载的包版本写入
  `provenance.software`，超出 `dependencies.lock.json` 范围的版本以 `warning` 诊断标出。

## Implemented capabilities / 已实现能力

| Capability | Packages | Notes / 说明 |
| --- | --- | --- |
| `sequence.stats.v1` | Biostrings (`readBStringSet`, `letterFrequency`, `width`) | length/N50/L50/auN/GC%/N% identical to the Rust `fasta_stats`; `BStringSet` so protein/RNA/IUPAC letters are accepted like the engine; gzip handled by Biostrings; the engine's error cases (no records, header without identifier, sequence before header) are reproduced. 统计定义与 Rust 一致；用 `BStringSet` 以接受蛋白/RNA/IUPAC 字母；gzip 由 Biostrings 处理；复刻三类报错。 |

Adding a capability: add `src/implementations/<capability>.R` defining an
`IMPLEMENTATION` list (`capability`, `input_roles`, `parameters`, `packages`,
`software()`, `run(inputs, parameters)`), extend `schemas/`, refresh
`manifest.json` with `python scripts/update-pack-manifest.py`, add the
capability to the pack entry in `workflows/catalog.json`, and add a parity
check against the Rust output to `tests/`.

新增能力：在 `src/implementations/` 放一个定义 `IMPLEMENTATION` 列表的 R 文件，扩展 `schemas/`，用
`scripts/update-pack-manifest.py` 刷新 `manifest.json`，在 `workflows/catalog.json` 的包条目里加上该
能力，并在 `tests/` 里补一条与 Rust 输出对照的一致性检查。

## Tests / 测试

```text
Rscript workflows/org.linxira.benchmark-r/tests/test_benchmark_harness.R
```

`jsonlite` and `digest` must resolve; Biostrings parity cases are skipped when
the package is missing. No network access is requested and no input is
uploaded.
