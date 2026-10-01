# Blog mirror — cloud-gpu-dual-vendor-validation (2026-10-02)

Handoff copies of the blog post for the product site, kept in-repo beside
the verification data the post references.

- `cloud-gpu-dual-vendor-validation.zh.md` / `.en.md` — authoritative source
  of the post (front-matter matches the site post format); the live-site
  copies are to be created from these files by the site pipeline
- `SHA256SUMS.txt` — hashes of the two markdown files; a mismatch between
  the live site and these hashes means the post was edited after handoff

Structure (both languages): the short version (5 findings) → 1 who ran
this and why (established community benchmarks only, nothing home-made)
→ 2 environment and machine performance → 3 what was proven (docking
budget line, signal injection, byte-level reproduction) → 4 our speed
against the traditional tool (Rust vs pytrf on three CPUs) → 5 the two
GPUs side by side → 6 two driver-layer facts → 7 reproduction →
8 boundaries → references (7 entries, DOIs where they exist) → version &
declarations.

Evidence base: docs/engine-evals/gpu-rental-2026-10-02.md (round 1),
gpu-rental-round2-s4000-2026-10-02.md (round 2),
gpu-rental-round3-dual-vendor-2026-10-02.md (round 3),
gpu-rental-round4-system-and-analysis-2026-10-02.md (round 4);
reproduction sources in scripts/rental/. Vendor listing prices, roadmaps,
and marketplace availability deliberately excluded (config-only policy).
