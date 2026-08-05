# 环境补全

## 用途

根据能力配置审计本地环境，安装缺失的 Python、R、Java 运行时和原生工具依赖。

## 输入

无需输入。通过 `--profile` 参数指定配置。

## 参数

`--profile` 选择配置（默认 `local-core`）。`--mode use-existing` 跳过安装仅报告状态。

## 输出

JSON 报告已安装、缺失和跳过的工具，并附各平台对应的安装命令。

## 示例

```bash
linxira-bio environment apply --profile local-core --json
```

## 结果解读

在批准安装前先审查缺失列表。使用 `--mode use-existing` 可在不修改系统的前提下审计。

## 注意事项

安装需要网络连接，可能需要系统包管理器凭据。应用前务必审查建议的变更。

## 运行时依赖

Python 3.10+、R 4.3+、Java 17+，以及系统包管理器（winget、apt、pacman）。

## 引用

应引用 Linxira Bio SDK 以及配置所安装的具体工具。

## 故障排除

若工具安装失败，请检查网络连接和包管理器配置。先使用 `--mode use-existing` 审计当前状态。