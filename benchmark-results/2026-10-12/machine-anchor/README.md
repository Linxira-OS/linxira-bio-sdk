# Machine-lineage anchor (2026-10-12): is this box ordinary?

Every absolute number in the benchmark archive is "on this machine". This record answers the
question a reader cannot answer from the archive: is that machine a lemon, a normal budget
box, or unusually fast? It is a ruler check, not a benchmark of the project.

Machine block (recorded in `machine.txt`): Intel Pentium Gold G7400, 4 threads (2 cores),
Linux 7.2.9-1-cachyos, CPU governor `powersave`, zstd 1.5.7, OpenSSL 3.6.5.

## Workload 1 — zstd, single-thread, 100 MB input

Input: the archive's own `big.fa`, 101,667,131 bytes (synthetic FASTA, 20 × 5 Mb). Protocol:
GNU `time -v`, three timed runs per level after the file cache was warm. Raw output:
`zstd-1*.txt`, `zstd-19.txt`.

| level | wall times (s) | median | throughput | output bytes | ratio | peak RSS |
| --- | --- | --- | --- | --- | --- | --- |
| `-1` | 0.42 / 0.41 / 0.43 | 0.42 s | **242 MB/s** | 32,847,929 | 0.323 | not recorded |
| `-3` | 0.47 / 0.44 / 0.44 | 0.44 s | **231 MB/s** | 33,238,950 | 0.327 | 45.1–47.1 MB |
| `-19` | 90.11 / 89.76 / 89.69 | 89.76 s | **1.13 MB/s** | 28,812,855 | 0.283 | 198.6–198.8 MB |
| decompress (`-1` output) | 0.09 / 0.10 | 0.095 s | **≈1,070 MB/s** | — | — | — |

Reference, from zstd's own benchmark table (README, `dev` branch, checked 2026-10-12):
`zstd 1.5.7 -1` — 510 MB/s compression, 1,550 MB/s decompression, measured with lzbench
(gcc 14.2.0) on the **Silesia corpus** on a **Core i7-9700K @ 4.9 GHz**. The table lists
level 1 and the negative levels only, which is why `-1` was run here as well: comparing our
`-3` against their `-1` would have been a level mismatch dressed up as a hardware result.

Reading: our `-1` compression is **0.47×** the reference and decompression **0.69×**, on an
input that is not Silesia. That is what a 2-core 2022 budget part at 3.7 GHz should look
like next to a 6-core 2018 desktop chip at 4.9 GHz — ordinary hardware, not a lucky or a
broken box.

## Governor, checked rather than assumed

`scaling_governor` is `powersave`, which invites the assumption that everything ran at
800 MHz. It did not: `scaling_cur_freq` reported **3,700,000 kHz** on cores 0–1 and
3,614–3,629 MHz on cores 2–3 under load (recorded in `zstd-level1-and-freq.txt`). The
disclosure stays in the article because a conservative governor is a reason to distrust
*headroom*, not a reason to distrust these numbers.

## Workload 2 — `openssl speed` sha256

`openssl speed -seconds 3 -evp sha256` (raw output in `openssl-sha256.txt`):

| block size | 16 B | 64 B | 256 B | 1 KiB | 8 KiB | 16 KiB |
| --- | --- | --- | --- | --- | --- | --- |
| throughput | 126.8 MB/s | 393.8 MB/s | 950.4 MB/s | 1,474 MB/s | **1,748 MB/s** | 1,736 MB/s |

At 3.7 GHz, 1.75 GB/s is ≈2.1 cycles/byte, which is the range expected when the CPU's SHA
extensions are in use. No published reference table is cited deliberately: OpenSSL does not
publish one, and inventing a "typical" figure would defeat the purpose of an anchor. The
number is here because it is a widely reproduced workload, so anyone can run the same
command on their own hardware and compare.

## Workload 3 — lzbench (partial: tool ready, corpus pending)

`lzbench` is the tool zstd's own reference table was measured with, which makes it the right
instrument for a like-for-like comparison. It is built here from source at revision
`9ca711ce574d1ac56b803db4292b8797283575b3` (lzbench 2.4.1, GCC 16.2.1; needs
`LD_LIBRARY_PATH` pointing at its build directory for `liblzbench_rust.so`), and its
invocation is validated on a 20 MB slice of our own input:

| selection | compress | decompress | ratio |
| --- | --- | --- | --- |
| `zstd 1.5.7 -1` | 242 MB/s | 954 MB/s | 32.33 |
| `zstd 1.5.7 -3` | 251 MB/s | 958 MB/s | 32.69 |

Cross-check worth noting: lzbench's `zstd -1` figure (242 MB/s) matches the GNU-time figure in
Workload 1 to the megabyte, by a completely different timing path — the two instruments agree
on this machine.

Still missing: the **Silesia corpus**, which the published comparison needs (zstd's table was
measured on Silesia with an i7-9700K). The official corpus lives on a university mirror
(`sun.aei.polsl.pl`), which is being fetched separately; GitHub-hosted mirrors are unusable
here (release and archive downloads stall or reset from both the server and the development
machine). Until it arrives, workload 3 is a validated instrument and not yet an anchor, and
nothing in this section is compared against published numbers.

## What this does not establish

- It does not validate the project's performance claims; nothing in the articles is derived
  from this record. It gives the machine a lineage.
- Workloads 3–5 of the plan (lzbench/Squash over Silesia, PTS `pts/compression` and
  `pts/bioinformatics` with an OpenBenchmarking upload) are **not** done: fetching those
  suites from the measurement host stalls or resets, and an upload is a public action that
  needs the maintainer's approval. They would add named, externally comparable results; this
  record adds the two cheapest ones.
- One input, one corpus, single-threaded. Compression on realistic multi-file data, or with
  I/O in the path, is a different measurement.
