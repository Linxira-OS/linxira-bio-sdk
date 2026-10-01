# Blog mirror — cloud-gpu-dual-vendor-validation (2026-10-02)

Handoff copies of the blog post for the product site, kept in-repo beside
the verification data the post references.

- `cloud-gpu-dual-vendor-validation.zh.md` / `.en.md` — authoritative source
  of the post (front-matter matches the site post format); the live-site
  copies are to be created from these files by the site pipeline
- `SHA256SUMS.txt` — hashes of the two markdown files; a mismatch between
  the live site and these hashes means the post was edited after handoff

Structure (both languages): TL;DR → changelog → 1 background (pairing axis)
→ 2 environment & methods → 3 results (3.1 kernel suite / bandwidth twin,
3.2 docking convergence & cross-vendor determinism, 3.3 storage & RAM,
3.4 analysis chain & five reproduction classes, 3.5 ecosystem observations)
→ 4 honest boundaries → 5 reproduction (scripts/rental/) → 6 engineering
records → 7 conclusion → references (7 entries, DOIs where they exist) →
version & declarations.

Evidence base: docs/engine-evals/gpu-rental-2026-10-02.md (round 1),
gpu-rental-round2-s4000-2026-10-02.md (round 2),
gpu-rental-round3-dual-vendor-2026-10-02.md (round 3),
gpu-rental-round4-system-and-analysis-2026-10-02.md (round 4);
reproduction sources in scripts/rental/. Vendor listing prices, roadmaps,
and marketplace availability deliberately excluded (config-only policy).
