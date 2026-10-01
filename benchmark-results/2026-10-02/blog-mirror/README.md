# Blog mirror — cloud-gpu-dual-vendor-validation (2026-10-02)

Handoff copies of the blog post for the product site, kept in-repo beside
the verification data the post references.

- `cloud-gpu-dual-vendor-validation.zh.md` / `.en.md` — authoritative source
  of the post (front-matter matches the site post format); the live-site
  copies are to be created from these files by the site pipeline
- `SHA256SUMS.txt` — hashes of the two markdown files; a mismatch between
  the live site and these hashes means the post was edited after handoff

Structure (both languages): the short version (4 findings) → 1 the three
scientific questions (correctness on a benchmark with an answer key;
sensitivity/specificity via signal injection; byte-level reproduction) →
2 docking: how much search budget is enough (e=8 loses the pose 1-in-3;
e>=32 never did; thread saturation explained) → 3 transcriptome
plant-and-recover → 4 the two GPUs side by side (same source, two vendor
compilers; bandwidth tie, atomics gap, ecosystem facts) → 5 reproduction →
6 boundaries → references (7 entries, DOIs where they exist) → version &
declarations. First draft was rewritten the same day after reader
feedback (jargon removed, science-first structure).

Evidence base: docs/engine-evals/gpu-rental-2026-10-02.md (round 1),
gpu-rental-round2-s4000-2026-10-02.md (round 2),
gpu-rental-round3-dual-vendor-2026-10-02.md (round 3),
gpu-rental-round4-system-and-analysis-2026-10-02.md (round 4);
reproduction sources in scripts/rental/. Vendor listing prices, roadmaps,
and marketplace availability deliberately excluded (config-only policy).
