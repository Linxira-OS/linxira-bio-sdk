# Python benchmark pack / Python 基准后端包

`org.linxira.benchmark-python` hosts the **independent Python implementations**
of Linxira Bio capabilities that also exist natively in the Rust engine. It is
the `python` backend of `linxira-bio benchmark run` (ROADMAP M2-T3) and the
home of the Python side of the three-way implementations planned for M3.

`org.linxira.benchmark-python` 收纳 Linxira Bio 各能力的**独立 Python 实现**——这些能力在
Rust 引擎中同样有原生实现。它是 `linxira-bio benchmark run` 的 `python` 后端（路线图
M2-T3），也是 M3 三端实现中 Python 一侧的落点。

## How it is invoked / 调用方式

The worker routes a request here when `execution.backend` is `"python"`:

```text
linxira-bio benchmark run sequence.stats.v1 fasta=tests/fixtures/sequences/tiny.fa \
  --backends rust,python --repeat 5 --output benchmark-results
```

Standalone (the request is a worker V2 request; `--result` must be
`<parameters.output_directory>/result.json`):

```text
python src/benchmark_harness.py --request request.json --result out/result.json
```

worker 在请求携带 `execution.backend = "python"` 时把整个请求交给本包；也可以脱离 worker
直接运行入口脚本（请求为 worker V2 格式，`--result` 必须位于
`parameters.output_directory` 之下）。

## Contract / 契约

* Input roles and parameters are **exactly** those of the native capability;
  the harness rejects anything the Rust contract does not accept, so the three
  backends stay interchangeable at the request level.
* The `result` object is field-for-field comparable with the Rust engine
  (`benchmark run` diffs it with a 1e-6 relative tolerance).
* One artifact, `<capability>.result.json` (kind `report`, format `json`),
  holds the result object; the envelope itself is `result.json`.
* An `info` diagnostic with code `benchmark.self_reported` carries the
  in-process wall time (`time.perf_counter` around the analysis only) and
  peak RSS (`resource.getrusage(RUSAGE_SELF)`, `null` on Windows). The outer
  `/usr/bin/time -v` measurement in the report includes interpreter start-up;
  the gap between the two is disclosed, not hidden.
* Installed library versions are recorded in `provenance.software`; a
  `warning` diagnostic flags drift from `requirements.lock` instead of
  failing, because benchmark servers legitimately differ from CI.

* 输入角色与参数与原生能力**完全一致**，Rust 契约不接受的内容一律拒绝，保证三端在请求层面可互换。
* `result` 对象与 Rust 引擎逐字段可比（`benchmark run` 以 1e-6 相对容差做 diff）。
* 产物固定一个：`<capability>.result.json`（kind `report`，format `json`），envelope 本身为
  `result.json`。
* `benchmark.self_reported` 这条 `info` 诊断携带进程内墙钟时间（仅包裹分析本身）与峰值
  RSS（`resource`，Windows 上为 `null`）。报告里的外层 `/usr/bin/time -v` 计时包含解释器启动，
  两者差值如实披露。
* 实际安装的库版本写入 `provenance.software`；与 `requirements.lock` 不一致时发 `warning`
  诊断而非失败，因为基准服务器与 CI 环境本就可能不同。

## Implemented capabilities / 已实现能力

| Capability | Library | Notes / 说明 |
| --- | --- | --- |
| `sequence.stats.v1` | Biopython `SimpleFastaParser` | length/N50/L50/auN/GC%/N% identical to the Rust `fasta_stats`; gzip detected by magic bytes; the engine's error cases (empty identifier, sequence before header, no records) are reproduced. 长度/N50/L50/auN/GC%/N% 定义与 Rust 一致，按魔数识别 gzip，复刻三类报错。 |

Adding a capability: drop a module into `src/implementations/`, register it in
`implementations/__init__.py`, extend `schemas/`, list the files in
`manifest.json` with their SHA-256, add the capability to the pack entry in
`workflows/catalog.json`, and add a parity test against the Rust output.

新增能力：在 `src/implementations/` 放一个模块并在 `__init__.py` 注册，扩展 `schemas/`，把文件及
SHA-256 列进 `manifest.json`，在 `workflows/catalog.json` 的包条目里加上该能力，并补一条与 Rust
输出对照的一致性测试。

## Tests / 测试

```text
python -m unittest discover -s workflows/org.linxira.benchmark-python/tests -p "test_*.py"
```

Biopython-dependent cases skip when the library is missing; CI installs
`requirements.lock`. No network access is requested and no input is uploaded.
