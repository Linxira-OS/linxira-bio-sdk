# M5-G1 CPU 基线（2026-09-29）

探针：`gpu-lab bench`（release 构建，固定种子合成数据——每次运行输入逐位一致，
checksum 可直接用于未来 GPU/SIMD 实现的一致性对照）。主机：Windows 桌面（Intel 核显
开发机，主机 A，见 gpu-hardware-2026-09-29.md）。

**SIMD 策略**：仅 x86-64（AMD 与 Intel 经运行时特性检测统一覆盖），分层
`avx512f → avx2+fma → scalar`；**明确不适配 ARM**（非 x86-64 仅 scalar 并在报告标注）。
高精度浮点内核（Pearson）是指令集适配主战场；哈希型（k-mer）与带宽型（直方图）内核
保持 scalar 并如实记录原因。

## Pearson 列对相关（f64，指令集分层对照）

布局：列主序 + 预中心化 + 每列平方和预计算（三实现共享同一连续输入，隔离出纯指令集差异；
相比 v1 的行主序 strided 布局，scalar 本身即从 669 ms 降至 ~230 ms——cache 效应 3 倍）。

| 实现 | wall | checksum |
| --- | --- | --- |
| scalar | 231–258 ms | sum-r=7.742407469 |
| avx2+fma（本机选中档） | 88–100 ms（**≈2.55×**） | sum-r=7.742407469（与 scalar 9 位小数一致） |
| avx512f | 本机无此指令集（Intel 消费核显平台），留云租 AMD Zen4/Zen5 或 Intel 至强验证 | — |

注：SIMD 分 lane 归约与标量顺序归约的重结合差在本负载下 <1e-9（打印精度内一致），
符合容差一致性判据；若在 AVX-512 机器上出现更大偏差则如实记录并复核。

## 整数内核（scalar，SIMD 不适用/暂缓）

| 内核 | 参数 | wall | 峰值 RSS | checksum |
| --- | --- | --- | --- | --- |
| kmer-count | 200k reads × 150bp，k=31，2400 万 distinct | 3,578 ms | **783 MB** | total=24000000 distinct=24000000 |
| quality-histogram | 3 亿碱基 Phred，256 bins | 130 ms | 296 MB | 20:14998548,21:15004945,… |

## 观察

1. **k-mer 内核是"内存驻留"叙事的锚点**：2400 万 distinct k-mer 的 HashMap 单线程峰值
   RSS 783 MB——"大量数据必须驻留内存"的负载形态，Rust 内存安全在该档位收益最直观；
   瓶颈在哈希表，SIMD 不适用（如实记录，不硬凑）。
2. **列主序布局的价值先于指令集**：布局优化（3×）与 AVX2（2.55×）相乘约 7.6×，
   GPU 对照（M5-G2）应以 SIMD 档为 CPU 参照，而非 strided 标量版。
3. quality-histogram 属带宽型（u32/直方图），是 WGSL 无 f64 限制下的纯适用内核；
   Pearson 的 f64 精度策略（f32 或 double-single）待 M5-G2 实测定论。

复现命令：`cd gpu-lab && cargo run --release -- bench`（报告含 per-implementation wall/checksum、
选中层与检测到的指令集）。
