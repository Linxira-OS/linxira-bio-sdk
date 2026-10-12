# Tool-arm round (2026-10-12): the first two ecosystem-tool arms

Parity arms answer "how much of this is the language runtime"; they cannot answer "what
would a user actually get", because they are our own hand-written implementations. This
round adds the second arm class with the mechanism that keeps the two apart (ROADMAP §7.1):
a separate producer (`scripts/benchmark/run/tool-arms.py`), an `arm_class: "tool"` field in
the report, and class guards in the table builder and the figure verifier.

## What ran

| arm | tool | version | input | N | median wall | IQR | CV | peak RSS |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `pyfastx-stat` | pyfastx | 2.3.1 | `big.fa` (100 MB, 20 × 5 Mb) | 10 | 30.0 ms | 0.0 ms | 0.00% | 17.36 MB |
| `r-prcomp` | R `prcomp` (stats) | R 4.6.1 | `matrix.tsv` (2000 × 40) | 10 | 150.0 ms | 20.0 ms | 6.63% | 81.93 MB |
| `py-sklearn-pca` | scikit-learn `PCA` | scikit-learn 1.9.1 | `matrix.tsv` (2000 × 40) | 10 | 725.0 ms | 70.0 ms | 6.77% | 146.34 MB |

The verbatim argv sits in each report (`tool.invocation`), `tool-arms.tsv` is written by the
table builder with `arm_class=tool` in every row, and the inputs are the same files the
parity window used — regenerated from the same seeds against `inputs.sha256`
(`big.fa 9f8a4cb4…`, `matrix.tsv 273755e5…`). Machine and environment are recorded in each
report; this is the same 2-core/4-thread host as the window, `powersave` governor, GNU `time`
present so peak RSS is measured rather than assumed.

## What they say, and why it is uncomfortable

The comparisons below cross the two arm classes, so they are stated in prose with both
classes named — never merged into one table (ROADMAP §7.1).

- **`pyfastx stat` reads the 100 MB FASTA in a median 30.0 ms.** The parity Rust arm for the
  same capability is **810 ms** on the same file: **27× slower**, while using less memory
  (5.9 MB against 17.4 MB). The tool builds a 29 KB `.fxi` index on first use; the warmup
  run absorbs that cost and the timed runs read the indexed file, which is how the tool is
  meant to be used. So the article's `sequence.stats` row is a statement about our scan, not
  about the achievable cost of reading a FASTA.
- **R `prcomp` takes 150.0 ms** on the matrix, against the hand-written R parity arm's
  2770 ms (**18.5× faster**) and the Rust parity arm's 20 ms (**7.5× slower**). The "R is
  slow" reading is therefore about the implementation we wrote in R, not about R.
- **scikit-learn's `PCA` takes 725.0 ms** — 5.5× faster than our Python parity arm (4010 ms)
  and 4.8× slower than R's `prcomp` on the same file, so at this size the fastest of the three
  is the R idiom, not the Python one. Recorded difference between the two arms: R's `prcomp`
  returned 40 components, scikit-learn 39 (the centred null direction is the difference); the
  ρ cost is dominated by the SVD either way, and both invocations are recorded verbatim so a
  reader can re-run them with a matched count.

Both are the answer to the question a user asks, and neither invalidates the parity arms,
which answer a different question. No claim is made here that the tools were used
optimally: the invocation is recorded precisely so that a better call can be substituted
and the difference attributed to the call rather than guessed.

## Reproduce

```bash
# on the measurement host, from the repository root
python3 scripts/benchmark/inputs/generate-inputs.py --set synthetic --out /tmp/lxn-tool-arms
python3 scripts/benchmark/inputs/generate-inputs.py --set sequence --out /tmp/lxn-tool-arms \
  --binary ./target/release/linxira-bio
rm -f /tmp/lxn-tool-arms/big.fa.fxi   # cold start: the warmup run rebuilds the index
python3 scripts/benchmark/run/tool-arms.py --tool pyfastx-stat \
  --input /tmp/lxn-tool-arms/big.fa --repeat 10 --output <dir>
python3 scripts/benchmark/run/tool-arms.py --tool r-prcomp \
  --input /tmp/lxn-tool-arms/matrix.tsv --repeat 10 --output <dir>
```

`--repeat` below 10 is refused unless `--allow-short` is passed, and a report is not written
at all if it fails validation against `schemas/benchmark-report.schema.json`.

## Not done in this round

- `seqkit`: **download succeeded; installation waits on the host.** Correcting an earlier claim
  in this file: I reported the download as failed, but I had checked
  `target/toolprov/seqkit.tar.gz` while the completed file was
  `target/toolprov/seqkit_linux_amd64.tar.gz` — 8,847,827 bytes, the exact size the GitHub API
  reports, containing a single `seqkit` member. Provenance is verified against the API's own
  asset digest: `sha256:3d664ffb48438d1fbd3f3cfc060c71d00368a26435f1c876944aea7fcdff86b5`, which
  matches the local file byte for byte, so no mirror trust is involved. The tarball is staged
  in `target/toolprov/` (gitignored) and the measurement host is currently unreachable
  (`ssh` timeout, no ping answer), so install + `seqkit version` + argv verification are
  pending host availability rather than anything else. No registry row exists until that
  verification happens, because the registry carries only argv checked against an installed
  version.
- `seqkit` is the *streaming* comparator to our own Rust scan (pyfastx indexes instead), so it
  is the arm that says whether our `sequence.stats` implementation is slow or whether streaming
  a FASTA is simply expensive. Worth getting right rather than replacing with another tool.
- `fastp`: **still a real decision.** It publishes no binary release assets at all, so it needs
  a package manager, a source build (C++ with isa-l/libdeflate), or a substitute. The
  pip-installable ecosystem standard for the same capability (adapter trimming) is `cutadapt`;
  swapping it in is the maintainer's call, not a silent substitution.
- Tool-arm figures and the article update: the article currently claims nothing from tool
  arms, so nothing published is wrong by omission; the finding above is the first candidate
  for a text change.
- Tool arms stay out of `statistics.tsv` by construction: mixing the classes in one table is
  what the rule forbids, and the builder now refuses it.
