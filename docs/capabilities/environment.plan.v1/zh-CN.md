# 环境计划

## 用途

根据审计结果和平台生成明确的安装计划，不改变计算机。

## 输入

无需数据文件；命令会在内部执行一次环境审计。

## 参数

`PROFILE` 可为 `local-core`、`scripting`、`managed-runtimes`、`containers`、
`sequence-search`、`genomics-cli` 或 `full-local`，默认值为 `full-local`。

`--mode` 可为 `use-existing`、`managed-user`、`project-isolated` 或
`system-missing-only`，默认为 `managed-user`。项目隔离模式必须提供
`--project-root PATH`。

## 输出

每项工具返回当前状态、选定的执行 provider、安装策略、包或运行时、规范来源、代理解析来源和管理员权限要求。

结果还包含只读事务预览：目标目录、缓存与锁文件路径、校验/许可证/原子激活策略、系统变更和管理员权限标记、计划阶段以及硬阻断项。结果结构遵循 `schemas/environment-plan.schema.json`。

## 示例

```bash
linxira-bio environment plan <profile> --mode <mode> --json
```

例如：

```bash
linxira-bio environment plan managed-runtimes --mode managed-user --json
linxira-bio environment plan sequence-search --mode project-isolated --project-root ./analysis --json
```

## 结果解读

`install` 表示目录中存在适用于当前平台的策略；它不表示安装器已经实现。

`missing` 表示 `use-existing` 模式没有找到可用命令，并且有意不提出安装；`unsupported` 表示当前平台或模式无法安全使用已登记策略。所有模式都保留已发现工具，`system-missing-only` 只规划缺失项。

`alternative` 表示互斥的执行后端，必须先选择其中一个，才能把结果视为安装事务。

Windows 上的 Unix 原生 genomics 操作会复用现有 WSL Arch 或 WSL Debian，并通过 `execution_provider` 返回选择结果。若尚无后端，`containers` 配置列出的是备选项，只选择其中一个，不是全部安装。

## 注意事项

此能力严格只读。`environment.apply.v1` 仍为计划功能，因此 `apply_available` 和 `ready_to_apply` 均为 false，不能把计划静默翻译成系统命令。系统级计划不等于执行授权。

## 运行时依赖

仅依赖 Linxira Bio CLI。GitHub 链接可通过受信任的 `GITHUB_PROXY` 解析。

## 引用

计划来自 `tools/catalog.json`，结果结构见 `schemas/environment-plan.schema.json`，事务要求见 `docs/RUNTIME_MANAGEMENT.md`。

## 故障排除

若配置名称未知，请先运行 `linxira-bio environment plan --json` 或查看工具目录中的 `profiles`。若项目隔离模式失败，请提供非空 `--project-root` 路径。
