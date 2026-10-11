# 第三方基准候选清单（2026-10-04 调研）

> 目的：把"我们自己跑分"升级为**第三方托管/第三方真值**的对照，避免自证。
> 判定口径：第三方拥有测试定义、真值集或执行基础设施，结果公开可复核。

## 候选总表

| 序 | 第三方基准 | 它衡量什么 | 我们提交/接入什么 | 工作量 | 价值 | 风险 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **GA4GH Benchmarking / GIAB 高置信集 + hap.py**（ga4gh/benchmarking-tools、Illumina/hap.py、NIST GIAB RM） | 变异检出**准确性**：与 NIST 参考材料真值逐位比对（SNV/INDEL/SV），输出 precision/recall/F1 | 用 `variant.call.v1` 在 GIAB HG001/HG002（如 v4.2.1 高置信区）上出 VCF，交 hap.py 评估 | 中（需下载真值集 + 参考基因组；算力 CPU 即可） | **最高**：唯一给出"我们的检出质量 vs 行业标准真值"的权威数字；学术界与面试都认 | 数据下载体积大（数十 GB 级）；需固定参考版本口径 |
| 2 | **OpenBenchmarking.org / Phoronix Test Suite**（第三方 test profile 体系，含 Scientific Computing 套件） | 系统级性能：同一 test profile 在任意机器/Distro 上跑，结果上传公开库 | 发布一个 test profile（XML + 安装/运行脚本），封装 `linxira-bio` CLI + 固定输入（如 `simulate.*` 生成的数据做对齐/统计/k-mer） | 中（写 profile + 一次跑通上传） | **高**：自带第三方基础设施与公开结果页；与"我们做 Arch 系发行版"叙事天然契合（Distro 基准是它的主场） | Profile 打磨需时间；需 Linux（正合 Linux 优先） |
| 3 | **nf-core / Nextflow 社区流水线**（第三方编写的标准流程） | 端到端流水线行为与耗时（fastq→qc→align→variant→annotate） | 用同一输入分别跑 nf-core 流程与我们的 CLI 链，做成"同输入对照"台账 | 中高（需 Nextflow + 容器环境） | 中高：证明我们在**真实流程语境**可比 | 容器/网络依赖重；口径需仔细对齐 |
| 4 | **Benchmarks Game**（benchmarksgame 语言级基准，含 k-nucleotide / fasta 等生物序列味道的题目） | 语言实现级性能（Rust vs C/C++/Go…） | 提交/对齐我们的等效实现（须先核对其现行提交入口与许可） | 低中 | 中：支撑"Rust 作为事实高性能语言"的公众叙事 | 与我们领域相关度低；口径是微基准 |
| 5 | **MLPerf（MLCommons）** | 训练/推理吞吐（AI 工作负载） | 仅当 AIVC/批次 G 的开放权重推理成形后才谈 | 高 | 低（当前无对应任务类别） | 现阶段不适配，暂不立项 |

## 推荐顺序与理由

1. **GIAB/hap.py 准确性基准（先做）**：投入/产出比最高，且是"第三方真值"而非"第三方跑分"。产出物为一页台账 + 可复核的 VCF/评估报告；对面试与后续论文都可直接引用。
2. **OpenBenchmarking PTS test profile（次做）**：把性能数据托管到第三方平台，形成公开结果页；同时服务发行版叙事（我们的 Arch 系基线在 PTS 体系下天然可比）。
3. **nf-core 同输入对照（第三档）**：更接近真实用户场景，但环境成本高，等前两项落地后按需启动。
4. Benchmarks Game / MLPerf：列为观察项，不占当前排期。

## 交付形态（每项都要满足）

- **口径固定**：参考版本/真值集版本/工具版本/种子全部写入台账；Linux 为唯一正式工况。
- **可复核**：原始输出（VCF/JSON/日志）与评估命令一并存档；第三方页面链接随结果记录。
- **红线沿用**：公开发文仅在用户批准的量化测试完成之后；结论只写实测能支撑的部分。

## 待办

- [ ] GIAB HG001 高置信区下载 + `variant.call.v1` 全链跑通（先核参考与真值版本对齐）
- [ ] hap.py 评估跑通并记录 precision/recall/F1 基线
- [ ] 起草 PTS test profile（封装 CLI + 种子化输入），本机跑通后上传 OpenBenchmarking
- [ ] 核对 Benchmarks Game 现行提交入口（许可与流程）再决定是否参与
