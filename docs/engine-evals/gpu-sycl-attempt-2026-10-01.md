# gpu-lab — sycl-rs Windows 集成尝试（2026-10-01）

> §3.1 批次 D 第 3 项 / M5-G3（提案 docs/proposals/m5-g3-vendor-track-plan.md §1.3、
> §2.1）。上游 oneapi-rs（sycl-rs v0.0.9，crates.io，2026-08-12）官方仅测试 Linux；
> 本页记录 Windows 实测的三层探测结果——**失败即为上游贡献素材**（提案 §2.1 的
> issue 内容由此页生成）。

## 环境

- 主机 A（Windows）：Core Ultra 5 225H + Arc 130T；oneAPI 2025.3 全套已装
  （DPC++ 位于 `C:\Program Files (x86)\Intel\oneAPI\compiler\2025.3\bin\icpx.exe`）。
- Rust 工具链：stable-x86_64-pc-windows-gnu（本仓库 CI 阵地，默认）；
  stable-x86_64-pc-windows-msvc（对照，本机未装 VS Build Tools）。
- 尝试方式：gpu-lab 可选依赖 `sycl-rs = "0.0.9"` + feature `gpu-sycl-attempt`
  （默认关闭，不进任何常规构建）。

## 三层探测结果

| 层 | 配置 | 结果 |
| --- | --- | --- |
| 1 | 默认 `cargo build --features gpu-sycl-attempt` | `sycl-rs-sys` build.rs panic：`Expecting a compiler. Set the ONEAPI_CXX environment variable.: No DPC++ compiler found` —— 上游对 Windows 无自动探测，需要手动指认编译器 |
| 2 | `ONEAPI_CXX=<icpx.exe>` + **windows-gnu** rust 目标 | icpx 被成功调起并带 `-fsycl` 编译，死于 C++ 头解析：`sycl/exception.hpp` 报 `unknown type name 'int32_t'` 等 **11 个错误**（cc-rs 传 `--target=x86_64-pc-windows-gnu`，icpx 的 MSVC 兼容头在此目标下失效） |
| 3 | `ONEAPI_CXX=<icpx.exe>` + **windows-msvc** rust 目标 | 未到达 sycl-rs-sys：rust 侧链接先死于本机缺 VS Build Tools（PATH 上的 GNU coreutils `link.exe` 接到 MSVC 链接参数报 `extra operand`）——需装有 Build Tools 的主机才能验证该分支 |

## 最小重现（上游 issue 素材，提案 §2.1 直接引用）

1. Windows + oneAPI 2025.3；`cargo build --features gpu-sycl-attempt`（依赖
   sycl-rs 0.0.9）→ 层 1 panic。
2. `set ONEAPI_CXX=C:\Program Files (x86)\Intel\oneAPI\compiler\2025.3\bin\icpx.exe`
   + rust 目标 x86_64-pc-windows-gnu → 层 2 的 11 个 exception.hpp 错误
   （cc-rs 命令行完整保留在构建输出：`icpx -O3 ... --target=x86_64-pc-windows-gnu
   -std=c++17 -fsycl -c types-sys.rs.cc`）。
3. 询问项：sycl-rs-sys 的 build.rs 是否计划支持 Windows 自动探测（vsystream/
   oneAPI setvars），以及 gnu 目标是否在支持矩阵内、还是强制 MSVC。

## 结论

- **windows-gnu（本仓库 CI 阵地）明确不可用**，失败点在 icpx 头文件解析层，
  非无法调起——这比"完全不支持"更有信息量：编译器能跑，缺的是目标配置。
- MSVC 分支待装有 VS Build Tools 的机器验证；若通过，则 sycl-rs 的 Windows
  支持形态为"MSVC-only"，与本仓库 windows-gnu CI 的关系需要上游答复。
- 本机 Arc 130T 的 oneAPI/SYCL 主开发栈结论（§9.5）不因此改变：Linux（上游
  正式配置）路径保持优先，Windows 数据一律临时值。

## 后续

1. issue 提交（提案 §2.1 文本 + 本页重现步骤），M5-G3 验收线"Intel 至少 1 个
   上游 issue"即达成。
2. 明天 Linux 服务器到位后，在该机重跑同 feature（上游正式配置），预期通过
   编译并接入 `SyclBackend` stub 的 is_available 探测。
