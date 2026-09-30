# 新人前沿分析快速通道（提案）

> 状态：提案文档，尚未接入任何 skill 或能力。若采纳，正式落位
> `docs/fast-track/`（面向新人的常设文档，与能力页分离）。目标读者：学术圈与
> 商业圈的研究生、初级工程师——目标是**用第一性原理做前沿分析**，而不是把工具
> 当黑盒调包。

## 0. 一句话总纲

本 SDK 的每个能力都是确定性可复现的——种子固定、结果 schema 版本化、fixture
可重跑。这条工程承诺不只是质量管控，更是学计算生物学的最快路径：**当结果变了，
一定是你改了什么，而找出"是什么"才是真正的本事。**

## 1. 应用层：按问题选能力，跑通完整管线

三个入口：

- `AGENTS.md` — skill 路由表：每行把一个实际问题（"我有一批 ONT 测序的
  FASTQ"）映射到一个 skill 文件夹。
- `skills/<name>/SKILL.md` — 简洁的操作流程：真实命令 + 解读指引。
- `docs/capabilities/<capability-id>/en-US.md`（及 `zh-CN.md`）— 十节固定结构：
  用途、输入、参数、输出、示例、结果解读、注意事项、运行时依赖、引用、故障排除。

经验法则：**问 AGENTS.md 找 skill，读 SKILL.md 拿流程，读能力页懂语义。**

### 示例链 A —— 从合成参考到变异检测（全本地，零下载）

```bash
# 1. 造一个可复现的玩具参考和读段（种子固定 => 字节级一致）
linxira-bio simulate sequence ref.fa --count 2 --length 20000 --gc 45 --seed 7
linxira-bio simulate reads ref.fa reads.fq --read-length 150 --coverage 15 --seed 7

# 2. 读段质检
linxira-bio fastq qc reads.fq --json

# 3. 比对（能力内部跑 minimap2 -x sr + samtools sort）
linxira-bio alignment short-read ref.fa reads.fq aligned.bam --threads 4

# 4. 标记重复（内部四步：collate/fixmate/sort/markdup）
linxira-bio alignment markdup aligned.bam marked.bam --stats

# 5. 变异检测（内部 bcftools mpileup+call）
linxira-bio variant call marked.bam ref.fa calls.vcf --min-mq 20 --min-bq 13

# 6. 汇总
linxira-bio variant stats calls.vcf --json
linxira-bio alignment coverage marked.bam coverage.tsv
```

每一步都输出版本化 JSON 信封（`capability`、`status`、`warnings`）——认真读
warnings，那是数据失效模式的免费教学。

### 示例链 B —— 相似性 → 注释 → 富集

```bash
# 找同源序列（原生 BLAST+ 编排）
linxira-bio similarity blast query.fasta reference.fasta hits.tsv --program blastn

# 把 GO 注释归一化成关联表
linxira-bio annotation go annotations.tsv associations.tsv

# custom/GO/KEGG 超几何富集与 preranked GSEA
linxira-bio enrichment custom genes.txt associations.tsv --json
linxira-bio enrichment gsea ranked.tsv gene_sets.tsv --json
```

### 示例链 C —— 合成基准闭环

```bash
linxira-bio simulate sequence ref.fa --count 1 --length 100000 --seed 42
linxira-bio simulate reads ref.fa reads.fq --read-length 150 --coverage 30 --paired --error-rate 0.5 --seed 42
linxira-bio alignment short-read ref.fa reads.fq aligned.bam
linxira-bio variant call aligned.bam ref.fa calls.vcf
# calls.vcf 与你掌握的真值对比——模拟器的全部意义：跑管线之前你就知道答案。
```

## 2. 硬件层：gpu-lab 就是教学载体

`gpu-lab/` 是独立 Cargo 工程（workspace 外、CI 外），存在的意义就是让新人
可以安全地弄坏东西：

1. **`probe`** — 枚举适配器、驱动与限制（M5-G0）。在你自己的机器上跑，输出
   就是一条硬件台账记录。
2. **CPU 基线**（`bench`）— 确定性 CPU 内核：SplitMix64 种子输入 + 运行时
   SIMD 档位检测（AVX-512/AVX2/scalar）（M5-G1）。在这里你会明白**基线是契约
   不是形式**：benchmark 门控没有基线就拒绝内核。
3. **wgpu/WGSL 内核**（`gpu-bench`）— 把基线移植到 WGSL，分相计时
   （upload/compute/readback），3 次取中位（M5-G2）。先读
   `docs/engine-evals/gpu-wgpu-2026-09-30.md`：它是台账的格式模板，包括逐指标
   精度容差的显式声明方式。
4. **厂商栈对照**（M5-G3，见 `docs/proposals/m5-g3-vendor-track-plan.md`）—
   同一内核走 cutile-rs / cudarc-musa / sycl-rs，trait 层负责回退 wgpu。

成长路径 = 里程碑阶梯本身：M5-G0 → G3。你的实验笔记就是 `docs/engine-evals/`——
测量纪律（≥3 次取中位、分相计时、容差显式）正是要学的技能。

## 3. 上游层：加入早期 Rust-GPU 社区

各家厂商 Rust 栈都是数天到数月的新事物；来自真实工作负载的首批实测报告能获得
维护者关注，并积累个人可见度：

- **cudarc-musa**（摩尔线程官方 fork）— 首批开发者窗口 issue（见提案 §2.2）。
- **cutile-rs**（NVIDIA 官方）— 消费级 sm_89 工作负载报告。
- **sycl-rs**（Intel oneapi-rs）— 上游未测 Windows；你的 Windows 实验本身就是
  贡献。

一份带最小可复现示例的认真 issue，胜过十个点星仓库——这也是新人能产出的最
便宜的可见成果。

## 4. 第一性原理 → 工程实践的翻译

| SDK 实践 | 照做你能学到什么 |
| --- | --- |
| 固定种子（`--seed`） | 可复现是特性；bug 变得可二分定位 |
| 结果 schema 版本化（`*.v1`） | 接口即契约；升版本是语义行为 |
| 可重跑 fixture（`tests/fixtures/`） | 黄金文件教会你精确的预期行为 |
| benchmark 门控（无 CPU 基线不写内核） | 性能声明从实测参照开始，而非直觉 |
| 双实现硬规则（CPU+GPU） | 没有可信回退的加速器是负资产 |
| 3 次取中位、分相计时台账 | 测量噪声是数据的一部分，不是麻烦 |

## 5. 建议的第一周

1. 跑 `linxira-bio doctor` 和环境审计；按建议的 profile 装一个缺失原生工具。
2. 通找示例链 A；逐个读 JSON 信封。
3. 在自己机器上跑 `gpu-lab probe` 和 `bench`；按 engine-evals 格式写两段
   台账。
4. 挑一份 SKILL.md 改进一句话——这就是你的第一次上游贡献。

---
*English version: see `en-US.md` in this folder.*
