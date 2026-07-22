# 本地任务 Worker

## 用途

通过统一的 JSON 请求调用已注册本地能力，供工作流、SDK 和 Agent 使用。

## 输入

一个符合 `schemas/job-request.schema.json`（旧版 v1）或
`schemas/job-request-v2.schema.json`（artifact-aware v2）的 UTF-8 JSON 文件。
v2 输入声明稳定的文件 ID、路径、格式、压缩、大小和可选 SHA-256。

## 参数

唯一命令行参数是请求文件路径。相对输入路径以请求文件所在目录为基准解析。

## 输出

标准输出返回与请求版本一致的 `AnalysisResult` JSON。身份完整的 v2 请求在语义
校验或执行失败时返回 `status: error` 和错误诊断；损坏 JSON 或无法可靠确定
schema、任务 ID 与能力 ID 的请求才写入标准错误并返回非零退出码。

## 示例

```bash
linxira-bio-worker <job-request.json>
```

仓库测试示例为 `linxira-bio-worker tests/fixtures/jobs/sequence-stats.json`。

## 结果解读

成功结果包含能力 ID、任务 ID、结构化结果和执行来源；v2 还记录输入哈希和输出
artifact 哈希。调用方必须检查 `status` 和 diagnostics，不能把传输成功当作科学
分析成功，也不应解析人类文本。

## 注意事项

当前 Worker 仅支持 `local-cpu` 和明确注册的能力；计划功能和远端模式会被拒绝。
v2 会检查输入基数、文件 ID 唯一性、大小、可选 SHA-256、内容可判定时的格式与
压缩声明，并检测执行期间发生变化的文件。

## 运行时依赖

依赖 Linxira Bio Worker 可执行文件以及所调用能力明确声明的运行时。

## 引用

请求与结果契约定义在 `linxira-bio-protocol` 和仓库 `schemas/` 中。

## 故障排除

若提示不支持 schema 或能力，请核对 `schema_version`、`capability` 和能力目录中的
状态。v2 请求应先读取 `job-failed` diagnostic，再检查进程 stderr。
