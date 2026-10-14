# B站视频 01：我们跟你在用的工具比了，我们输了

- 时长目标: 10–12 分钟；形式: 录屏 + 字幕卡；分辨率 1920×1080，终端字号 ≥ 18
- 一句话定位: 一份自己宣布失败的基准测试，以及它凭什么可信
- 前置素材: `benchmark-results/2026-10-11/`、`benchmark-results/2026-10-12/tool-arms/`、
  `benchmark-results/2026-10-12/machine-anchor/`、`REPRODUCE.md`、
  `docs/outreach/corrections-log.md`
- 所有旁白中的数字都取自上述归档；录制时如数字有更新，改旁白而不是改归档

## 分镜

| # | 画面 | 旁白（要点，非逐字稿） | 屏幕内容 |
| --- | --- | --- | --- |
| 1 | 封面卡 | — | 「我们跟你在用的工具比了，我们输了」；副标题：2 核旧机器 · N=10 · 输入可逐字节复现 |
| 2 | 三行结果卡 | 三行数字先说结论：读 100 MB FASTA，`pyfastx` 30.0 ms，我们手写的 Rust 实现 810 ms（工具侧取 2026-10-12 会话、手臂侧取 2026-10-11 窗口） | 三行大字号对比 + 「27×」（字幕注明：改前读数，我方缺陷已修复） |
| 3 | 终端录屏 | 交代机器：Pentium Gold G7400，2 核 4 线程，15 GiB，governor `powersave`（负载下实测 3.7 GHz） | `cat /proc/cpuinfo \| head`, `nproc`, `cat .../scaling_cur_freq` |
| 4 | 终端录屏 | 演示生成输入并校验 sha256：「输入是生成的，不是下载的——所以任何人能在自己机器上复现」 | `python scripts/benchmark/inputs/generate-inputs.py --set sequence --out /tmp/x`，`sha256sum` 对照 `inputs.sha256` |
| 5 | 表格卡 | 三语言全景（parity 臂）：`sequence.stats` 810 / 825 / 1815 ms；`expression.pca` 20 / 4010 / 2770 ms | 归档 `statistics.tsv` 截图，逐行高亮 |
| 6 | 字幕卡 | 关键澄清：parity 臂是**同一算法三种实现**，只测语言运行时边界，不代表用户会拿到什么 | 「同算法三实现 ≠ 生态工具」 |
| 7 | 终端录屏 | 真实 tool arm：`pyfastx stat big.fa` 30.0 ms（10 次，IQR 0，RSS 17.4 MB） | tool-arm 报告 JSON + `tool-arms.tsv` 行 |
| 8 | 字幕卡 | 第二个失败：我们手写的 R 幂迭代 2770 ms，R 的 `prcomp` 150 ms（18.5×）；而我们写了"R 慢"的结论 | 两行对比 + 文章截图 |
| 9 | 字幕卡 | 第三个：scikit-learn 725 ms vs 我们手写 Python 4010 ms（5.5×） | 三臂对照表 |
| 10 | 终端录屏 | 我们如何防止自己骗人：报告里带 `arm_class`，两类臂**禁止同表**；`statistics-tables.py` 直接报错 | 演示混排被拒（故意放一个 tool 报告进 parity 目录，看它 exit≠0） |
| 11 | 归档卡 | 更正台账：把今天这四个错（lock 装不上、测试期望写错、GNU time 缺失、governor 名字）念出来 | `docs/outreach/corrections-log.md` 滚动 |
| 12 | 锚点卡 | 这台机器不是神机也不是烂机：zstd -1 242 MB/s，对照官方表 510 MB/s（0.47×，i7-9700K @4.9 GHz） | 锚点表 + 官方页引用 |
| 13 | 复现卡 + 结尾 | 「五分钟就能反驳我们」：clone → 跑 `reproduce.sh --smoke` → 对照归档的 `inputs.sha256` 逐字节一致；对不上就开 issue | 命令 + issue 模板名 |
| 14 | 收尾 | 下一期预告：一台 2 核旧电脑跑完整生信流程；以及我们的能力面有 132 项 | 一行仓库名 + 一句"错的地方会进更正台账" |

## 旁白禁忌（自检清单）

- 不出现「最强 / 第一 / 碾压 / 吊打」；倍数一律带机器、输入与 N。
- 不把 parity 臂的数字说成"生态工具的性能"；两类的句子不混在一句里。
- 不说"我们没有短板"；本期主旨就是认输。
- 医学相关能力一律加"研究用途"。

## 录制前检查

1. 终端里 `uname -r`、`nproc`、governor 三项在一屏内可见（后面对齐机器参数）。
2. 本地待在仓库 tip；归档里记录的 `code_revision` 是 `89882a5a…`（视频里展示这次校验即可）。
   **不要**为了跑脚本把工作区切到那一版：它早于 `reproduce.sh` 与归档文件本身。
3. 故意混排被拒的演示：用 `--tool-arms` 与 parity 目录做一次，确认报错信息会显示在屏幕上。
4. 视频里引用的每个数字，录制后再对着归档核一遍（旁白错一个数字，整个系列的可信度就打折）。

## 简介模板（发布时填）

```
仓库：<repo 链接>
五分钟自证：REPRODUCE.md → scripts/benchmark/reproduce.sh --smoke
本视频引用的归档：
  benchmark-results/2026-10-11/（三语言窗口，N=10）
  benchmark-results/2026-10-12/tool-arms/（pyfastx / prcomp / scikit-learn）
  benchmark-results/2026-10-12/machine-anchor/（机器谱系锚点，2026-10-12 轮）
  benchmark-results/2026-10-13/（锁环境整窗重跑 + 四条 tool arm）
机器：Intel Pentium Gold G7400（2 核 4 线程）、15 GiB、governor powersave（负载下 3.7 GHz）
许可：代码 AGPL-3.0-or-later；第三方工具各按其自身条款
我们自己的更正台账：docs/outreach/corrections-log.md
```
