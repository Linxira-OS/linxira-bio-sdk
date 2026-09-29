# M5-G1 CPU 基线（2026-09-29，第一份）

探针：`gpu-lab bench`（单线程，release 构建，固定种子合成数据——每次运行输入逐位一致，
checksum 可直接用于未来 GPU 实现的一致性对照）。主机：Windows 桌面（Intel 核显开发机，
主机 A，见 gpu-hardware-2026-09-29.md）。

| 内核 | 参数 | wall | 峰值 RSS | checksum |
| --- | --- | --- | --- | --- |
| pearson-column-pairs | rows=2000 cols=500（124,750 列对，f64） | 669 ms | 16.6 MB | sum-r=-1.619673722 |
| kmer-count | 200k reads × 150bp，k=31，2-bit 打包，HashMap | 3,535 ms | **783 MB** | total=24000000 distinct=24000000 |
| quality-histogram | 3 亿碱基 Phred，256 bins | 102 ms | 296 MB | 20:14998548,21:15004945,…（均匀 ~1500 万/bin） |

观察：

1. **k-mer 内核是"内存驻留"叙事的锚点**：2400 万 distinct k-mer 的 HashMap 在单线程下峰值
   RSS 783 MB——正是"大量数据必须驻留内存"的负载形态，Rust 内存安全（无 C++ 越界/悬垂/
   泄露导致的任务中断）在该档位收益最直观。
2. Pearson 124,750 列对仅 669 ms：单线程已快，GPU 侧的对照意义在"同能耗/同墙钟下的吞吐
   放大"与核显档位（主机 A）能否追平或超过——这决定"最弱硬件跑出性能"叙事的成立面。
3. quality-histogram 属带宽型（u32/直方图），是 WGSL 无 f64 限制下的纯适用内核；Pearson
   的 f64 精度策略（f32 或 double-single）待 M5-G2 实测定论。
4. 基线为**单线程**：M5-G2 起 GPU 版与 CPU 版对照须同输入同种子；未来 Rayon 多线程版
   （M5-T3 路线）另列一档，不与本基线混算。

复现命令：`cd gpu-lab && cargo run --release -- bench`。
