# M5-G0 GPU 硬件台账（2026-09-29，第一份）

探针：`gpu-lab probe`（wgpu 30.0.1，Vulkan/DX12/GL 全后端枚举 + 4096 元素 compute 冒烟）。

## 主机 A（Windows 桌面，Intel 核显开发机）

| 项 | 值 |
| --- | --- |
| GPU | Intel Arc 130T（核显，16GB 共享内存，Core Ultra 平台） |
| 可用后端 | Vulkan（adapter 0）、DX12（adapter 1）、GL（adapter 4）+ WARP 软件回退 |
| compute 冒烟 | **通过**（Vulkan 后端，4096/4096 元素全对，f32 变换核验证） |
| max_buffer_size | Vulkan 4 GiB / DX12 2 GiB |
| max_storage_buffers_per_shader_stage | Vulkan 3,355,442 / DX12 262,144 / GL 16 |
| max_compute_invocations_per_workgroup | 1024（三后端一致） |

结论：**核显开发目标成立**——Vulkan 后端为首选（buffer 上限与 storage buffer 绑定数最高），
DX12 可作 Windows 备用，GL 后端 storage 绑定仅 16 个、只作枚举参考。16GB 共享内存对
"大矩阵驻留内存"类内核（M5-G1 Pearson/k-mer）是充足的上限起点。

## 待测卡（后续补充）

| 卡 | 状态 | 预期路径 |
| --- | --- | --- |
| NVIDIA GTX 750 Ti（Maxwell，2GB） | 待测 | wgpu/Vulkan 1.1（唯一存续路径；与 CUDA 13 栈断代） |
| AMD RX 580（Polaris，8GB） | 待测 | wgpu/Vulkan（RADV/AMDVLK 支持良好） |
| 摩尔线程 S80 / S90（16GB） | 待购/待租 | cudarc-musa（官方 Rust fork）+ wgpu/Vulkan 1.3 |
| 云租 RTX 4090（sm_89） | 待租 | cutile-rs（NVIDIA 官方 Rust，Tile 轨） |
| 云租 RX 9060/9070 XT | 待租 | ROCm/HIP 官方栈 + wgpu/Vulkan |
| 云租昇腾 910B | 待租 | 社区 cann-rs |

复现命令：`cd gpu-lab && cargo run --release -- probe`（JSON 报告含全部 adapter 与冒烟结果）。
