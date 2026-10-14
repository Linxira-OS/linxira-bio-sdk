# We benchmarked ourselves against the tools you already use, and we lost

> **DRAFT — not posted anywhere.** Publication waits for the maintainer's date. The plan,
> targets and rules of engagement are in `criticism-request-plan.md`; the claim-by-claim
> checklist is in `reviewer-brief.md`; our own error ledger is in `corrections-log.md`.

---

## English

We published a three-language benchmark (Rust / Python / R) of bioinformatics capabilities,
measured on one deliberately modest machine — 2 cores / 4 threads, 15 GiB, CPU governor set
to `powersave` — with a reproduction package (`REPRODUCE.md`), a stated list of known gaps,
and an append-only ledger of our own errors (`corrections-log.md`). Before anyone cites it,
we would rather find out where it is wrong.

We just ran the second class of comparison arms — the ecosystem tools a user would actually
reach for — on the same machine and inputs. We lost:

| arm | our own parity arm | ecosystem tool | result |
| --- | --- | --- | --- |
| `sequence.stats` on a 100 MB FASTA | Rust, 810 ms, 5.9 MB RSS | `pyfastx stat` 2.3.1, **30.0 ms**, 17.4 MB | the tool is **27× faster** |
| `expression.pca` on a 2000 × 40 matrix | R, 2770 ms (hand-written power iteration) | R `prcomp`, **150.0 ms** | the idiom is **18.5× faster** |

We record these losses rather than hide them, and we label what they are: cross-class
comparisons, where the two sides do not produce the same output set (the `pyfastx` row is an
indexed read against our scan; the PCA row is a library against our hand-written implementation).
Our boundary rule - call an existing implementation when it is already faster **on the same job**
- only applies once a same-job comparison exists, and for these two rows it does not yet. Measuring
that is the open work item; until then the loss stands as a recorded number, not as a verdict about
whose implementation a job should use. (See the README's "On Rust" section for the boundary rule.)

So the sentence "R is slow" in our article was about the implementation *we* wrote, not about
R; and our `sequence.stats` row is a statement about our scan, not about what reading a FASTA
can cost. Both readings were in the text as cross-language claims. That is the class of
mistake we want pointed out, and we would rather point out the first instance ourselves.

### The three places we would attack us

1. **The parity arms are not the tools anyone uses.** They are the same algorithm written
   three times, which isolates the language runtime and says nothing about what a user gets.
   We now measure both classes and label them per row, and we never put them in one table —
   but a reviewer should still ask whether the parity comparison was worth doing at all.
2. **One small machine, one file at a time.** 2 cores, tmpfs, `powersave` governor,
   single-file inputs. If your pipeline is 32 cores behind Nextflow with a distributed
   filesystem, our numbers may be irrelevant to you. We say so in the article's limitations;
   say it louder if you think we understated it.
3. **No third-party anchor, no truth-set gate yet.** We have not run PTS, OpenBenchmarking,
   lzbench or Squash, and we have not compared a variant call against a GIAB truth set. Both
   are planned (the rental plan for the second one is public). Until then, every number comes
   from our own harness, and you should treat its protocol as the thing under test.

### Five minutes, if you want to check us

```bash
git clone <this repository> && cd <it>
git checkout 89882a5a31714e12e0dee4333d20f2b50b0c99c5   # the archived revision
scripts/benchmark/reproduce.sh --smoke                   # inputs must regenerate byte-identically
```

Then `docs/outreach/reviewer-brief.md` lists the four checks (inputs reproducible, arms
consistent within 1e-6 over the whole result object, medians inside the archived bootstrap
intervals, figures decodable back to the archive) and what we would accept as falsification.
If something fails on your machine, the issue form asks for the revision, the command, both
outputs and your environment — those four decide whether the difference is ours or the
machine's.

### What we are asking for

Counterexamples, not encouragement. Specifically:

- **Tool authors** (`pytrf`/`pyfastx`, Biostrings, Biopython, htslib): is the way we called
  your tool the way you would call it? The verbatim argv is recorded in every tool-arm
  report, so a better call can be substituted and the difference attributed to the call
  rather than guessed.
- **Practitioners**: would any of these capabilities be used alone in your work, or is the
  single-file benchmark shape the wrong unit entirely?
- **Reviewers of the framing**: is "parity arm" a useful category, or does it just launder
  our implementation choices into a language comparison?

We will answer within 48 hours, log every objection in the correction ledger — including the
ones we reject, with reasons — and never argue a number against a measurement: if you report
a number that contradicts ours, we reproduce yours first.

If this thread fills with praise, we posted in the wrong place, and we will move it.

---

## 简体中文

我们发布了一份三语言（Rust / Python / R）生物信息学能力基准：在一台刻意简陋的机器上测量
（2 核 4 线程、15 GiB、CPU governor 固定 `powersave`），配了可复现包（`REPRODUCE.md`）、
明写的已知缺口清单，以及只增不删的**自我更正台账**（`corrections-log.md`）。与其被引用之后
才发现哪里错了，不如现在先把它问倒。

我们刚在同一台机器、同一批输入上跑了第二类对照臂——**用户真正会用的生态工具**。结果是我们输：

| 对照臂 | 我们自己的 parity 臂 | 生态工具 | 结果 |
| --- | --- | --- | --- |
| 100 MB FASTA 上的 `sequence.stats` | Rust 810 ms、5.9 MB | `pyfastx stat` 2.3.1、**30.0 ms**、17.4 MB | 工具**快 27×** |
| 2000×40 矩阵上的 `expression.pca` | R 手写幂迭代 2770 ms | R `prcomp`、**150.0 ms** | 惯用法**快 18.5×** |

也就是说，文章里"R 慢"那句话讲的是**我们写的那个 R 实现**，不是 R；`sequence.stats` 那一行讲的
是我们的扫描实现，而不是读 FASTA 的可达成本。这两句原本是作为跨语言结论写出来的。这正是我们
希望被指出的错误类型——而第一个例子，我们自己先指出来。

### 如果换成我们来攻击自己，会打这三处

1. **parity 臂不是任何人用的工具**：三份同算法实现只能分离语言运行时，回答不了"用户拿到什么"。
   我们现在两类臂都测、逐行标注、绝不混排于一张表；但审稿人依然该问：这个对照本身值不值得做。
2. **一台小机器、一次一个文件**：2 核、tmpfs、`powersave`、单文件输入。如果你的流水线是 32 核
   Nextflow 加分布式文件系统，我们的数字可能与你无关。限制章节已写明；若你觉得写得还不够重，
   请说得更响。
3. **还没有第三方锚点，也没有真值集门禁**：PTS、OpenBenchmarking、lzbench、Squash 都没跑；
   变异检测也还没和 GIAB 真值集比过。两者都在计划内（后者的租机方案已公开）。在此之前，所有
   数字都出自我们自己的测量框架——请把框架本身当作受检对象。

### 五分钟就能反驳我们

```bash
git clone <本仓库> && cd <它>
git checkout 89882a5a31714e12e0dee4333d20f2b50b0c99c5   # 归档修订版
scripts/benchmark/reproduce.sh --smoke                   # 输入必须逐字节重现
```

随后 `docs/outreach/reviewer-brief.md` 列出四项检查（输入可复现、三臂在整份结果对象上 1e-6 内
一致、中位数落在归档 bootstrap 区间内、图能反解回归档）以及我们承认的证伪条件。若在你机器上
对不上，issue 模板会问四件事：修订版、命令、两边原始输出、环境——这四项决定了差异是我们的还是
机器的。

### 我们要的不是鼓励

我们要反例。具体来说：

- **工具作者**（`pytrf`/`pyfastx`、Biostrings、Biopython、htslib）：我们调用你的工具的方式，
  是你会采用的方式吗？每份 tool arm 报告都记录了逐字 argv，所以更好的调用可以直接替换，差异能
  归因到调用而不是靠猜。
- **实际使用者**：这些能力在你的工作里会单独使用吗？还是"单文件基准"这个形状本身就是错的单位？
- **对框架有意见的人**："parity 臂"是有用的分类，还是只是把我们的实现选择洗成语言对比？

我们承诺：48 小时内回应；每一条反对意见都进更正台账——包括我们拒绝的，并写明理由；绝不拿数字
跟测量争辩——你报出与我们矛盾的数字时，我们先复现你的。

如果这个帖子最后只收获赞美，说明我们发错了地方，我们会换场地。
