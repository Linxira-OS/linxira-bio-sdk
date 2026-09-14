# Release format assessment: AppImage (Linux), zip+installer (Windows), source-only (macOS)

> Purpose: decide the shipping formats for the SDK binaries, what each
> format must contain, which special package builds are needed, and the
> concrete CI work to get there.
>
> 发行格式评估：Linux 用 AppImage、Windows 出 zip 便携版 + exe 安装器、
> macOS 不出二进制（源码自建），并评估特定包构建需求。

## 0. What we already have

- `scripts/stage-release.py` + `packaging/bundle-manifest.json`: stages
  sources (docs/schemas/skills/workflows/licenses) + three platform binaries
  (linxira-bio, -worker, -ui) into a staging root, writes sha256 locks; CI
  already runs it for Windows.
- The Rust binaries are self-contained for **their own** functionality;
  delegated native tools (salmon, fasterq-dump, samtools, …) are discovered
  at runtime and reported by `environment audit` — release packages must
  not pretend to bundle them.

## 1. Linux — AppImage (x86_64) plus tar.gz

| aspect | decision |
|---|---|
| format | AppImage (primary), `.tar.gz` of the same AppDir payload (fallback for distros restricting FUSE) |
| build base | **debian-bookworm CI job** — oldest glibc we support; AppImage runs fine on newer glibc (CachyOS/Arch included, as our own lab host proves) |
| AppDir layout | `usr/bin/{linxira-bio,linxira-bio-worker,linxira-bio-ui}` + `usr/share/linxira-bio/{docs,schemas,skills,workflows,licenses}`; `.desktop` entry for the GUI |
| tooling | linuxdeploy (bundles the few non-base `.so` deps) + appimagetool; both run headless in CI |
| external tools | NOT bundled (salmon/kraken2/… stay per-distro or conda); `environment audit` is the first-run guide |
| special builds | release profile adds the `zlib-ng` flate2 feature; future `sra-native` feature ships as a separate `-full` AppImage once the SRA streaming lands (keeps the main image lean) |

## 2. Windows — portable zip + exe installer (x86_64-windows-gnu)

| aspect | decision |
|---|---|
| portable zip | current staging output + `README-FIRST.txt` (environment audit pointer); no admin rights needed |
| installer | **Inno Setup** (scriptable, Windows-runner friendly, signed-checksum output): installs binaries + resources under `%LOCALAPPDATA%\LinxiraBio`, Start-Menu shortcuts for the GUI, optional PATH edit |
| GUI specifics | egui app — no WebView dependency; NotoSansCJK font already staged for CJK text |
| external tools | same policy: not bundled; `environment audit` + docs point to conda/WSL installs |
| special builds | zlib-ng feature on; `sra-native` stays Linux-only initially (ncbi-vdb Windows support is weak) — installer notes disclose this |

## 3. macOS — no binaries (source build)

Documented path only: `rustup` + `cargo build --release -p linxira-bio-cli`
(+`-p linxira-bio-ui`), native tools via brew/conda. README already names
macOS as untested-for-packaging; keep it that way until a maintainer with
hardware volunteers.

## 4. What the packages must NOT contain

Raw user data, benchmark results, `.research/` clones, private paths — the
staging manifest already enforces the allowlist; keep it that way.

## 5. Concrete next actions (CI wiring)

1. `stage-release.py`: add `--appdir` mode producing the AppDir layout
   (reuse the manifest; no new sources).
2. debian-bookworm CI job: after tests, build AppDir → linuxdeploy →
   appimagetool → upload artifact `linxira-bio-<ver>-x86_64.AppImage` +
   sha256; same job emits the `.tar.gz`.
3. windows-gnu CI job: stage → zip (portable) → Inno Setup compile
   (`packaging/windows-installer.iss`) → upload
   `linxira-bio-<ver>-setup.exe` + zip + sha256.
4. Release automation: on tag `v*`, attach all four artifacts to the GitHub
   release (the manual `gh release create` we do today becomes the tail of
   this pipeline).
5. Later: `-full` Linux variant with `sra-native`; `.deb`/`.rpm`/AUR only if
   users ask.

## 中文摘要

**Linux**：AppImage（主）+ 同内容 tar.gz（备用），基于 debian-bookworm 构建
拿最老 glibc 兼容新系统（我们 CachyOS 实验机已实证可跑）；AppDir 装 3 个二
进制 + 文档/schema/skills 资源 + GUI 桌面项；**外部工具不打进去**（环境审计
引导安装）；release 启用 zlib-ng。**Windows**：zip 便携版 + Inno Setup 安装
器（装 %LOCALAPPDATA%、开始菜单快捷方式、可选改 PATH）；egui GUI 无
WebView 依赖，中文字体已随包。**macOS**：不出二进制，README 给源码自建路
径。**特定包构建**：`sra-native`（Linux -full 变体）、zlib-ng（release 默认
开）。**CI 五步**：stage 加 --appdir 模式 → debian job 出 AppImage+tgz →
windows job 出 zip+setup.exe → tag 自动挂 release → 之后按需 .deb/AUR。
