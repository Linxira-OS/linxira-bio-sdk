---
title: "把一个有标准答案的药物对接实验，搬到两台不同厂商的 GPU 服务器上"
date: 2026-10-02
tag: technical
lang: zh
desc: "Linxira Bio SDK 云端实测：在有官方答案的 HIV-1 蛋白酶对接实验上，搜索预算给到多少才够（e=8 有三分之一次会丢掉正确构象，e≥32 从不丢）；在藏了差异基因和富集信号的模拟转录组里，分析链能否把信号完整找回来；以及换一台完全不同厂商的服务器，结果是否一个字节都不差。"
---

# 把一个有标准答案的药物对接实验，搬到两台不同厂商的 GPU 服务器上

> Linxira Bio SDK 云端验证报告 · 2026-10-02。我们租了两台服务器——一台
> 装 NVIDIA RTX 4080 SUPER，一台装摩尔线程 MTT S4000——把 SDK 的几条
> 分析链放上去跑了一遍。这篇文章记录三个问题的答案，环境与命令全部
> 披露，复现脚本随仓库发布。

## 先说结论

1. **分子对接的搜索预算有明确的"够用线"**：在官方教程的经典体系（HIV-1
   蛋白酶 + 茚地那韦）上，默认的 exhaustiveness=8 三次里有一次找不到
   正确构象（偏差 12 Å）；给到 32 以上，三次全部落在正确构象 0.07 Å
   以内。快筛用 8，定结论用 32，这是我们给出的操作建议。
2. **分析链能从数据里把人为藏进去的信号完整找回来**：我们在模拟转录组
   里藏了 150 个差异基因和一条定向富集的通路，五步分析链（质控→标准化
   →PCA→聚类→富集）把它以 p = 6×10⁻³¹ 的显著性挖了出来，其余通路都在
   背景水平——没有漏报，也没有误报。
3. **换厂商、换 CPU、换核数，结果一个字节都不差**：对接、SSR 微卫星
   挖掘、表达谱五步链、富集分析——五类输出文件在两台机器上的 SHA256
   完全相同。对一个宣称"本地优先、结果可复现"的工具，这是最硬的证据。
4. **国产卡能跑我们的东西**：同一份 GPU 内核源码，摩尔线程的 mcc 编译
   器一次就编译通过、结果算对，内存带宽利用率与国际卡同档（都有改进
   空间的地方，详见第四节）。

## 一、我们在回答什么问题

这次测试不是为了跑分。我们是做生物信息工具的，关心的是三件每个
使用者都会遇到的事：

**第一，工具算得对不对？** 对接是最合适的考题，因为它有标准答案：
AutoDock Vina 官方教程用 HIV-1 蛋白酶和茚地那韦（PDB 条目 1iep [3]）
做演示，官方跑出的最优结合能是 -13.234 kcal/mol，构象文件也随教程
发布。我们的工具链从同一个受体和配体出发，能不能回到同一个位置？

**第二，分析链灵不灵？** 转录组分析没有单一"标准答案"，所以我们用
信号注入的办法：在模拟数据里藏已知的差异基因和富集通路，看工具能否
精确找回。找不回是漏报，找回之外的"发现"是误报。

**第三，结果能不能复现？** 如果同一份输入在两台不同厂商的机器上算出
不同的结果，下游所有的统计分析都会被这个问题污染。我们想知道的是
最严格的版本：文件级、逐字节一致。

## 二、对接实验：搜索预算要给多少

**红停靠（redocking）是什么**：把实验解出的复合物拆开，只保留蛋白
和配体的结构，让对接程序从随机位置重新把配体"摆"回去。摆得回去，
说明打分函数和搜索算法在这类体系上是可靠的——这是虚拟筛选前的
例行检验 [2,4]。

我们扫了 exhaustiveness（搜索预算，Vina 里大致等于并行起的独立
搜索次数）× 随机种子，共 12 组，在两台机器上各跑一遍。下面的表是
最优构象与官方参考构象的重原子偏差（单位埃）：

| 搜索预算 | 种子 42 | 种子 43 | 种子 44 |
| --- | --- | --- | --- |
| 8（默认） | 0.058 | 0.861 | **12.42（结合能 -10.9，正确构象丢了）** |
| 16 | 0.058 | 0.861 | 0.015 |
| 32 | 0.062 | 0.068 | 0.015 |
| 64 | 0.056 | 0.047 | 0.045 |

读法：1-2 埃以内算"回到了同一口袋"；12 埃意味着分子跑到了别的位置，
但程序自己不知道——结合能 -10.9 看起来也"不错"。这正是 Monte Carlo
搜索的陷阱：不是报错，而是安静地给你一个次优解。

**给使用者的建议**：快速筛大量分子时用默认的 8 没问题（丢解是随机的，
排序大体保留）；但凡这个对接结果要写进结论，预算给到 32 以上。

顺带一个和直觉相反的观察：线程数加到 8 以上就没用了（72 秒 → 13 秒
后持平）。原因很朴素——e=8 就是 8 条独立搜索链，8 个线程刚好一对一，
再加线程没有链可分。想快，加预算不如接受它就这么多。

## 三、转录组分析：藏信号，找信号

我们构造了一组模拟表达谱：2000 个基因、12 个样本（6 对照 6 处理），
泊松计数模拟测序深度波动。其中 150 个基因被人为设定了上调或下调
（1 到 2 倍），并让一个通路在这些差异基因里定向富集。

五步链全部通过我们的 Rust CLI 完成（质控 → median-ratio 标准化 [6]
→ PCA → 双向聚类 → GO 超几何富集 [7]）：

- 藏进去的通路以 **p = 6.3×10⁻³¹** 排在富集结果第一位（44 个基因命中）；
- 其余 25 个通路 p 值全部大于 0.03，没有假阳性冒头；
- PCA 和聚类都把 12 个样本按处理/对照干净分开（这类模拟数据应该分开，
  分不开才说明管线有问题）。

这套"信号注入—回收"的办法会进入我们的常规验证流程：它比"跑通了
没报错"严格得多，又不需要真实临床数据。

## 四、两块 GPU 上的对照

对接和转录组分析主要吃 CPU。GPU 部分我们做的是另一件事：**同一段
内核代码，分别用两家的编译器编译，看结果对不对、速度差多少**。四个
内核（拷贝、triad、直方图、相关性计算）全部通过正确性校验（位级或
对 f64 参照误差 < 10⁻⁴）：

| 内核 | RTX 4080 SUPER | MTT S4000 | 备注 |
| --- | --- | --- | --- |
| 拷贝 1 GiB | 630 GB/s | 474 GB/s | 单向读写，NVIDIA 快三分之一 |
| triad（两读一写） | 666 GB/s | 711 GB/s | 最接近标准内存带宽口径 [5]，两家基本打平 |
| 直方图（原子操作） | 26.6 GB/s | 6.5 GB/s | MUSA 的原子吞吐目前明显落后 |
| 相关性矩阵 | 282 GB/s | 49 GB/s | 见下 |

用大白话总结：**这两块卡的内存带宽在同一档**（都在各自标称值的九成
左右），跑以数据搬运为主的生信内核是等价的；差距在原子操作这类细粒
度同步上，摩尔线程落后约四倍——写 MUSA 内核时应尽量把原子操作换成
分桶归并。相关性内核两家都不快，因为我们这版没做合并访存（两家同罚，
比值有意义，绝对值没有）。

**生态层面的两个事实**（中性记录，证据都在仓库台账里）：

- 摩尔线程的 mcc 编译器拿到 CUDA 风格的源码直接就编过了，三次微调
  （加链接参数等）后内核一次算对。对我们来说，这意味着 MUSA 后端
  没有工具链障碍。
- 两家的 Vulkan 驱动各有一层问题（NVIDIA 的 vGPU 驱动缺 Vulkan 入口，
  摩尔的驱动加载成功但初始化失败），所以走 Vulkan 的跨平台 GPU 路线
  在这两台云实例上暂时都不通——我们已把日志证据留档，也计划反馈给
  厂商。这不影响上面的结果：内核走的是各家原生编译器。

## 五、你怎么复现

复现脚本随仓库发布（`scripts/rental/`），租任何一台有对应 GPU 的
Linux 机器即可：

```bash
git clone https://github.com/Linxira-OS/linxira-bio-sdk.git
cd linxira-bio-sdk && cargo build --release -p linxira-bio-cli

# 对接研究（12 组扫描 + RMSD 分析）
bash scripts/rental/dock-study.sh
python3 scripts/rental/analyze_dock.py <bundle>/docking <study_dir>

# GPU 内核对照（同一份源码，两家编译器）
mcc -O3 scripts/rental/bench_suite.mu -o bench_mu -lmusart
cp scripts/rental/bench_suite.mu b.cu && nvcc -O3 -arch=sm_89 b.cu -o bench_cu

# 转录组信号注入验证 + 存储/内存带宽
bash scripts/rental/rna_analysis.sh
bash scripts/rental/system_bench.sh
```

原始输出文件（JSON/TSV/日志/哈希）在本地归档目录 `release-artifacts-rental/`
（不入库）；逐轮实验台账在 `docs/engine-evals/`。

## 六、边界（照例说清楚）

- 每张卡只有一台云租实例、一个时间窗；云实例的 vCPU 和内存带宽受宿主
  调度影响，**墙钟时间只代表那台实例，不代表厂商规格**。
- 磁盘读速是页缓存口径（容器无权清缓存）；对接结论只对官方 1iep 体系
  成立，其他体系需自行验证。
- e=8 丢解是 Monte Carlo 搜索预算的固有性质，不是 Vina 或我们的缺陷；
  它的用处是提醒你给自己定一条预算线。

## 参考文献

[1] Williams, S., Waterman, A., Patterson, D. Roofline: an insightful
visual performance model for multicore architectures. *Commun. ACM*
52(4), 65–76 (2009). doi:10.1145/1498685.1498715

[2] Trott, O., Olson, A. J. AutoDock Vina: improving the speed and
accuracy of docking with a new scoring function, efficient optimization,
and multithreading. *J. Comput. Chem.* 31(2), 455–461 (2010).
doi:10.1002/jcc.21334

[3] Berman, H. M. et al. The Protein Data Bank. *Nucleic Acids Res.*
28(1), 235–242 (2000). doi:10.1093/nar/28.1.235

[4] Eberhardt, J., Santos-Martins, D., Tillack, A. F., Forli, S.
AutoDock Vina 1.2.0: New docking methods, expanded force field, and
Python bindings. *Nucleic Acids Res.* 49(W1), W3355–W3362 (2021).
doi:10.1093/nar/gkab663

[5] McCalpin, J. D. STREAM: Sustainable Memory Bandwidth in High
Performance Computers. Technical Report, University of Virginia
(1991–2007). https://www.cs.virginia.edu/stream/

[6] Anders, S., Huber, W. Differential expression analysis for sequence
count data. *Genome Biol.* 11, R106 (2010). doi:10.1186/gb-2010-11-10-r106

[7] Ashburner, M. et al. Gene Ontology: tool for the unification of
biology. *Nat. Genet.* 25(1), 25–29 (2000). doi:10.1038/75556

## 版本与声明

- **版本**：2026-10-02 首发（本版为重写稿：初版发布当天按读者反馈
  全文重写，改为以科学问题为骨架、面向使用者的叙述）。
- **自评**：作者自报的验证报告，非第三方独立验证；单实例单窗口，
  边界见第六节。第三方复现为后续工作。
- **许可**：代码 AGPL-3.0-or-later；本文 CC-BY-4.0。
- **作者与溯源**：Linxira-OS 项目维护者 · 仓库：
  <https://github.com/Linxira-OS/linxira-bio-sdk>

---

*Linxira-OS · AGPL-3.0-or-later（代码）· 本文 CC-BY-4.0 ·
<https://github.com/Linxira-OS/linxira-bio-sdk>*
