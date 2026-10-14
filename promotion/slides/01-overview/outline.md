# 幻灯片 01：总览 / 演示（12 页）

- 用途：面试、课程分享、展会/路演、内部对齐；可在 B站 视频中复用为字幕卡
- 原则：每页一个主张；数字必须在同页标注机器、输入规模与 N；不出现绝对化词
- 素材：`capabilities/catalog.json`、`benchmark-results/2026-10-11/`、
  `benchmark-results/2026-10-12/`、`REPRODUCE.md`、`docs/outreach/corrections-log.md`

| # | 标题 | 一句话主张 | 页面元素 | 数字来源 |
| --- | --- | --- | --- | --- |
| 1 | 本地优先的生信工具链 | 一台普通电脑、一个 CLI、可复现 | 一句话定位 + 3 个数字（132 能力 / 46 skills / 36 tools） | `capabilities/catalog.json`、`skills/`、`tools/catalog.json` |
| 2 | 问题：生信分析被"环境"绑架 | 装环境比做分析慢，结果还复现不了 | 三段式对比：conda 地狱 → 云端成本 → 结果不可复现 | 无（问题陈述，不引数字） |
| 3 | 做法：能力 = 编排原生工具 | 不重造算法，重造执行与契约 | bcftools/samtools/BLAST+/DIAMOND/HMMER/Kraken2/salmon/MUSCLE/trimAl/MEME/IQ-TREE/Vina/Meeko/RDKit | `tools/catalog.json` |
| 4 | 执行模型 | 本地执行、确定性输出、版本化 schema | CLI 三层（能力/工作流/报告）+ 132 项能力清单示例 | `capabilities/catalog.json` |
| 5 | Benchmark 方法学 | 我们把"怎么测"当产品做 | 机器块 + N≥10 + bootstrap CI + 10 ms 时钟口径 + 输入 sha256 | `benchmark-results/2026-10-11/inputs.sha256` |
| 6 | 三语言实测（parity 臂） | 同算法三实现，测的是语言运行时边界 | 两行对照：`sequence.stats` 810/825/1815 ms；`expression.pca` 20/4010/2770 ms | `statistics.tsv` |
| 7 | 生态工具对照（tool arm） | 用户真正拿到的是什么 | pyfastx 30.0 ms（27× 快于我们，**改前读数**）、R `prcomp` 150 ms、scikit-learn 725 ms | `tool-arms.tsv` + 2026-10-14 台账 |
| 8 | 我们输在哪 | 承认失败是方法的一部分 | 三行失败 + 一句"文章里'R 慢'指我们写的 R 实现" | 同上 + 文章勘误 |
| 9 | 可复现性 | 五分钟自证，而不是相信 | `reproduce.sh --smoke` 输出（9/9 输入 sha256 一致）+ issue 模板 | `REPRODUCE.md` |
| 10 | 自我更正台账 | 只增不删，含一次厂商说法撤回 | 三条代表错误（锁文件、内存未测、撤回） | `docs/outreach/corrections-log.md` |
| 11 | 机器谱系锚点 | 这台机器普通，不是神机也不是烂机 | zstd -1 242 MB/s vs 官方表 510 MB/s（0.47×） | `machine-anchor/` |
| 12 | 路线与边界 | 下一步做什么、不做什么 | M5 并发/IO 方向（BGZF 并行、解码后端优先）+ 研究用途声明 + AGPL/商用双路径 | `ROADMAP.md` §9.7、`COMMERCIAL.md` |

## 讲稿提示（每页一句过渡）

1→2：「先说为什么会有这个东西。」 2→3：「所以我们的做法是编排，而不是重写算法。」
3→4：「编排要能被人信，就得有契约。」 4→5：「契约里最难的是'怎么测'。」
5→6：「先看同算法三实现。」 6→7：「但用户不会用我们的实现，他们用生态工具。」
7→8：「于是就有了这一页。」 8→9：「输了还怎么让人信？靠可复现。」
9→10：「以及靠认错。」 10→11：「还有一件事要堵：有人说你这机器特殊。」
11→12：「最后是边界和路线。」

## 制作注意

- 第 6、7 页必须分开展示（两类臂禁止同表），口播也要分开说。
- 第 8 页的三行失败数字，与第 7 页共用同一份归档；不要另做一套口径。
- 导出 `.pptx` 后放进 `promotion/assets/`（已 gitignore），并在 `assets/MANIFEST.md` 记录。
- 对外演示若使用他人论文里的数字（如 zstd 官方表），必须在页脚给出来源与测量条件。
