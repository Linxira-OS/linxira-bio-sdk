# Promotion plan: 小红书 and B站 first

2026-10-12. Written for the maintainer to approve, edit or reject; nothing is posted from
here. `README.md` in this directory holds the rules every piece follows.

## What we actually have to promote

| Asset | Why it is promotable | Where the evidence lives |
| --- | --- | --- |
| One CLI, **132 available capabilities**, 46 skills, 36 tools, local-first, no cloud needed | "I can run a real analysis on my own machine tonight" | `capabilities/catalog.json`, `skills/` |
| A **three-language benchmark** with a reproduction package, a known-gaps list and a self-correction ledger | almost nobody publishes their own error list; that is the differentiator | `benchmark-results/2026-10-11/`, `REPRODUCE.md`, `docs/outreach/corrections-log.md` |
| **Honest losses**: pyfastx 27× faster than our Rust arm, R's `prcomp` 18.5× faster than the R arm we wrote, scikit-learn 5.5× faster than our Python arm | the hook that makes the rest credible. Labelled precisely: cross-class comparisons (different output sets), so recorded losses — not routing verdicts, and not a claim that one side wins everywhere. **Pre-fix readings**: our side of the two `sequence.stats` ratios contained a branch-misprediction defect, located and fixed on 2026-10-14 (see the self-correction ledger); the post-fix ratios wait for a re-measured window | `benchmark-results/2026-10-12/README.md` |
| A **machine-lineage anchor**: the box is ordinary, checked against zstd's own published table (0.47× / 0.69×) | pre-empts "your numbers are meaningless because your machine is weird" | `benchmark-results/2026-10-12/machine-anchor/` |
| Research-use-only medical capabilities and GPU work (docking, Vina, dual-vendor kernels) | a second audience: 生信/计算化学 students and interviewers | the GPU article and `docs/engine-evals/` |

## Platform playbooks

### 小红书 (image notes, mobile-first, layperson-adjacent)

- **Form**: 1 cover image + 5–8 content images, 300–600 字正文, 6–9 标签.
- **Cover**: one number and one line. Example: 「我们用 100 MB 的 FASTA 比了三种语言，结果被一个 pip 包按在地上」— the number must be from the archive.
- **Content images**: 归档表截图、图（12 张已校验的 SVG 之一）、命令 + 输出的对照、更正台账的一页。No stock imagery.
- **Tone**: 第一人称、讲结论、给出可运行命令；不写"最强/第一/碾压"，写"在这台机器、这批输入上，× 倍（N=10，CI …）"。
- **Tags**: `#生物信息学` `#生信` `#Rust` `#Python` `#性能测试` `#科研工具` `#开源` `#数据分析` `#大学生科研`（按篇微调，不堆砌）。
- **Cadence**: 2 篇/周 for the first month, alternating tracks (工具 / 系统), so the account does not read as one long benchmark ad.
- **Must include**: 机器参数 + N + 复现入口（GitHub 仓库名 + `REPRODUCE.md`）。

### B站 (screen-recording video, 5–15 min, technical audience)

- **Form**: 16:9 录屏 + 少量字幕卡；封面放大数字；标题含「实测 / 复现 / 从零」。
- **Series**: ①「我们跟你在用的工具比了，我们输了」(benchmark honesty) ②「一台 2 核旧电脑跑完整生信流程」 ③「可复现性怎么做：从 sha256 到更正台账」 ④「132 个能力里挑 5 个解决实际问题」。
- **Description**: 仓库链接、`REPRODUCE.md`、本视频引用的归档路径与命令、机器参数、以及一句"错的地方会出现在更正文台账里"。
- **Cadence**: 1 个/2 周, because each one needs a recorded run and a script that survives review.
- **Subtitles**: Chinese burned-in for key numbers; keep the terminal font large (viewers watch on phones).

## Content pillars and backlog

| # | Pillar | Track | Platform | Hook |
| --- | --- | --- | --- | --- |
| 1 | 三语言实测 + 生态工具对照 | 工具 | 小红书 ×2, B站 ① | 我们输了，而且我们自己先说出来 |
| 2 | 一台旧电脑跑完整流程 | 系统 | 小红书, B站 ② | 2 核 4 线程、15 GiB、powersave |
| 3 | 可复现性方法论 | 工具 | 小红书, B站 ③ | 输入逐字节可复现 + 我们自己的更正台账 |
| 4 | 能力面巡礼（挑 5 个真问题） | 系统 | 小红书 ×3, B站 ④ | 132 项能力，挑最常用的 5 个演一遍 |
| 5 | 面试/课程向：三语言差异到底在哪 | 工具 | B站 短片 | 同算法三实现，测的是运行时边界 |
| 6 | 医学/GPU 方向（研究用途） | 系统+工具 | B站, 知乎 | 双厂商 GPU 内核、对接 Vina 的 RMSD 对照 |

## Acceptance criteria for any piece

1. Every number has an archive path or a command; the piece itself states machine, input size
   and N.
2. At least one limitation is stated explicitly in the piece (not only in a link).
3. The reproduction entry point appears in the description or the notes' last image.
4. No absolute language ("最强", "第一", "guaranteed") and no clinical claims.
5. The maintainer signs off before posting; after posting, the piece is indexed in
   `README.md` with its platform URL and the date.

## What we deliberately do not do

- No paid promotion, no engagement buying, no cross-posting identical text across platforms.
- No "AI 一键出结果" framing: the system's point is deterministic local execution, and the
  medical capabilities are research-use-only.
- No claims about the commercial licence beyond "the same code can be licensed commercially";
  pricing is not a promotion topic.
- No leaderboard screenshots from other projects' benchmarks; we cite published tables
  (as the anchor does) or we measure ourselves.

## Storage and handoff

- Scripts, outlines and note texts: this directory (source of truth, reviewed like code).
- Recordings, exports, `.pptx`: `promotion/assets/` (gitignored) with `assets/MANIFEST.md`
  recording name, size, sha256 and where it is stored.
- Published articles: the site repository. Published notes/videos: the platform, with the URL
  backfilled into `README.md`'s index.
