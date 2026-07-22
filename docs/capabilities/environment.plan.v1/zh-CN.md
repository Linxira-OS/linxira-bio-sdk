# 环境计划

## 用途

根据审计结果和平台生成明确的安装计划，不改变计算机。

## 输入

无需数据文件；命令会在内部执行一次环境审计。

## 参数

`PROFILE` 可为 `local-core`、`scripting`、`managed-runtimes`、`containers`、
`sequence-search`、`genomics-cli` 或 `full-local`，默认值为 `full-local`。

## 输出

每项工具返回当前状态、安装策略、包或运行时、规范来源、代理解析来源和管理员权限要求。

## 示例

```bash
linxira-bio environment plan <profile> --json
```

例如：`linxira-bio environment plan managed-runtimes --json`。

## 结果解读

`install` 表示目录中存在适用于当前平台的策略；它不表示安装器已经实现。

## 注意事项

此能力严格只读。`environment.apply.v1` 仍为计划功能，不能把计划静默翻译成系统命令。

## 运行时依赖

仅依赖 Linxira Bio CLI。GitHub 链接可通过受信任的 `GITHUB_PROXY` 解析。

## 引用

计划来自 `tools/catalog.json`；事务要求见 `docs/RUNTIME_MANAGEMENT.md`。

## 故障排除

若配置名称未知，请先运行 `linxira-bio environment plan --json` 或查看工具目录中的 `profiles`。
