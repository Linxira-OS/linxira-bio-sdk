# 环境审计

## 用途

只读检查 Windows、Debian 或 Arch 工作站上的分析运行时、工具、WSL 和 GPU 条件。

## 输入

无需输入文件。审计读取当前操作系统和当前进程可见的命令。

## 参数

当前版本没有业务参数；`--json` 输出标准分析结果封装。

## 输出

返回平台信息、每个工具的命令与版本证据、可用/缺失数量、执行后端状态、Conda/Bioconda 配置和警告。

## 示例

```bash
linxira-bio environment audit --json
```

## 结果解读

工具只有在探测进程成功退出并满足可选文本匹配时才标记为可用。
Windows 以 WSL 或 Docker 任一可用为后端就绪；Debian/Arch 分别检查 Docker 和 Podman，任一可用即可。
Bioconda 不提供原生 Windows 包；Windows 上即使通道已经配置，也必须通过 WSL Debian 等 Linux 后端运行 Bioconda 环境。

## 注意事项

审计不会安装、升级、删除或修改环境变量，也不会判断数据库是否已经下载。Windows 上会在 PATH 探测失败后读取 R 注册表和已登记的 Conda 根目录，并明确标注这种定位结果。

## 运行时依赖

内置 Rust 审计器。Windows 上的 Unix 工具可由 WSL Debian 提供，但本命令不会创建 WSL。

## 引用

工具与探测参数来自 `tools/catalog.json`，执行安全边界见 `docs/EXECUTION_POLICY.md`。

## 故障排除

若 WSL 已存在但未识别，请确认 `wsl.exe --list --quiet` 中确实有名称包含 `Debian` 的发行版。
