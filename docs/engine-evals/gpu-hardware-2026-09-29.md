# M5-G0 GPU 硬件台账（2026-09-29，第一份）

探针：`gpu-lab probe`（wgpu 30.0.1，Vulkan/DX12/GL 全后端枚举 + 4096 元素 compute 冒烟）。

## 主机 A（Windows 桌面，Intel 核显开发机）

| 项 | 值 |
| --- | --- |
| CPU | Intel Core Ultra 5 225H（Arrow Lake-H，14C/14T）；**实测未暴露 AVX-512**（bench 检测仅 avx2+fma） |
| 内存 | 31.4 GB（M1+ 档） |
| GPU | Intel Arc 130T（核显，16GB 共享内存，**Xe-LPG+：含 DPAS/XMX 矩阵指令**） |
| 驱动 | 32.0.101.8991（2026-08-24） |
| **oneAPI** | **2025.3 完整套件已装**（compiler/dal/dev-utilities 等）；System32 存在 ze_loader.dll（Level Zero）与 OpenCL.dll |
| 可用后端 | Vulkan（adapter 0）、DX12（adapter 1）、GL（adapter 4）+ WARP 软件回退；**oneAPI/SYCL/Level Zero 为主开发栈，Vulkan 仅回退** |
| compute 冒烟 | **通过**（wgpu/Vulkan 后端，4096/4096 元素全对，f32 变换核验证） |
| max_buffer_size | Vulkan 4 GiB / DX12 2 GiB |
| max_storage_buffers_per_shader_stage | Vulkan 3,355,442 / DX12 262,144 / GL 16 |
| max_compute_invocations_per_workgroup | 1024（三后端一致） |
| FP64 | 经 cl_khr_fp64 暴露但比率低（~1/16-1/32，Xe-LPG 级）；GPU 侧 f64 内核采 f32/double-single 策略 |

结论：**本机是完整的 oneAPI 开发站（核显主栈 = SYCL/sycl-rs/Level Zero，非 Vulkan）**；
Arc 130T 的 Xe-LPG+ 带 DPAS 矩阵引擎，SYCL 矩阵负载可吃硬件加速。wgpu/Vulkan 保留为
回退与旧卡（750 Ti / RX 580）路径。sycl-rs 官方仅测 Linux——Windows 实测反馈即上游贡献点。

## 待测卡（后续补充）

| 卡 | 状态 | 预期路径 |
| --- | --- | --- |
| NVIDIA GTX 750 Ti（Maxwell sm_50，2GB） | 待测 | **双路径（2026-09-30 修正）**：wgpu/Vulkan + cudarc/CUDA 12.x（sm_50 仍受支持；Linux R580 驱动分支支持 Maxwell，仅老 Kepler 700 系落 490 legacy）；cutile-rs 不可用（需 CUDA 13.2+/sm_80） |
| AMD RX 580（Polaris，8GB） | 待测 | wgpu/Vulkan（RADV/AMDVLK 支持良好） |
| 摩尔线程 S80 / S90（16GB） | 待购/待租 | cudarc-musa（官方 Rust fork）+ wgpu/Vulkan 1.3 |
| 云租 RTX 4090（sm_89） | 待租 | cutile-rs（NVIDIA 官方 Rust，Tile 轨） |
| 云租 RX 9060/9070 XT | 待租 | ROCm/HIP 官方栈 + wgpu/Vulkan |
| 云租昇腾 910B | 待租 | 社区 cann-rs |

复现命令：`cd gpu-lab && cargo run --release -- probe`（JSON 报告含全部 adapter 与冒烟结果）。
