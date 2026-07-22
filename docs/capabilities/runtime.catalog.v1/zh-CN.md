# 运行时目录

## 用途

列出 Linxira Bio 计划管理的 Python、R、Java 和混合分析环境提供者。

## 输入

无需输入文件。

## 参数

使用 `--json` 返回完整、稳定的目录对象。

## 输出

返回提供者 ID、运行时类型、管理器、版本策略、平台、许可、来源和健康检查。

## 示例

```bash
linxira-bio runtime catalog --json
```

## 结果解读

`cataloged` 表示设计和许可边界已经登记，不表示当前版本能够安装该提供者。

## 注意事项

目录不会探测当前机器；机器状态使用 `environment.audit.v1`。目录也不会修改全局环境。

## 运行时依赖

仅依赖 Linxira Bio CLI。默认提供者为 uv、Pixi、rig 和 Eclipse Temurin；Miniforge 作为 Conda/Bioconda 兼容提供者登记。

## 引用

规范来源为 `runtimes/catalog.json`，模式为 `schemas/runtime-catalog.schema.json`。

## 故障排除

若 JSON 无法解析，请运行仓库验证器检查嵌入目录与 schema 版本。
