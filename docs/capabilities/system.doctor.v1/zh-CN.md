# 系统诊断

## 用途

快速确认 Linxira Bio SDK 运行平台和一组关键命令是否可用。

## 输入

无需输入文件。

## 参数

使用 `--json` 返回稳定的机器可读结果。

## 输出

输出操作系统、CPU 架构以及 Rust、Python 和常用本地工具的探测状态。

## 示例

```bash
linxira-bio doctor --json
```

## 结果解读

`available: true` 只表示探测命令成功执行，不表示任意分析流程的全部依赖已经满足。

## 注意事项

此命令保留早期诊断 JSON 结构。完整环境信息应使用 `environment.audit.v1`。

## 运行时依赖

仅依赖 Linxira Bio CLI；被检查的外部工具可以不存在。

## 引用

探测定义来自仓库内的 `tools/catalog.json`。

## 故障排除

若某工具已安装却未被发现，请从同一终端直接运行其探测命令，并检查当前进程的 `PATH`。
