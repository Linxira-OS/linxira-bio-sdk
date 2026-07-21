# Toolchain

## Supported Build Direction

The project does not require Visual Studio or the MSVC IDE toolchain. Windows is
the primary desktop release target.

- Windows: Rust GNU target with LLVM-MinGW or MSYS2 UCRT64, plus Ninja when C++
  kernels are introduced.
- Debian and Arch: Rust plus GCC or Clang from the native distribution packages.
- Project Rust baseline: Rust 1.92 or newer, installed independently of the
  distribution package when required by the native GUI stack.
- Future Windows C++: standalone CMake, Ninja, and LLVM/Clang or MinGW.
- CI: Windows GNU is primary; Debian Bookworm and Arch Linux are secondary
  release gates.
- macOS: no build, CI, packaging, or support commitment is made at this stage.

The current source tree is pure Rust and does not require CMake. Add CMake only
when the first benchmark-justified C++ kernel is introduced.

## Windows Prerequisites

Use a machine-level MSYS2 UCRT64 toolchain rooted outside the repository. The
validated local layout is `C:\Rust\msys64`, with `RUST_GNU_HOME` set to
`C:\Rust\msys64\ucrt64`. Install the Rust GNU **host** toolchain so build
scripts and procedural macros do not fall back to the MSVC host:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal `
  --component rustfmt --component clippy
rustup component add rust-src llvm-tools-preview rust-analyzer `
  --toolchain stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
```

Install the general native build layer in MSYS2 UCRT64: `base-devel`, the
`mingw-w64-ucrt-x86_64-toolchain` group, Git, CMake, Ninja, and pkgconf. Put
`%RUST_GNU_HOME%\bin` before any older MinGW directory on the user `PATH`, but
after the preferred Python distribution when Python precedence matters. Set
the Cargo GNU linker, `CC`, `CXX`, `AR`, and `PKG_CONFIG_PATH` to the UCRT64
tools. These are machine development settings, not repository-owned files.

Build explicitly with that toolchain:

```powershell
cargo +stable-x86_64-pc-windows-gnu build --workspace
cargo +stable-x86_64-pc-windows-gnu test --workspace
```

`scripts/test-windows-gnu.ps1` reads `RUST_GNU_HOME` and runs the workspace
checks and smoke tests. A different UCRT64 or LLVM-MinGW root can be supplied
explicitly:

```powershell
./scripts/test-windows-gnu.ps1
./scripts/test-windows-gnu.ps1 -GnuToolchainHome C:\msys64\ucrt64
```

No Visual Studio installation, solution, MSBuild generator, or MSVC linker is
part of the supported Windows path.

When C++ is added, configure it without an IDE generator:

```powershell
cmake -S engine/cpp -B build/cpp -G Ninja `
  -DCMAKE_C_COMPILER=gcc `
  -DCMAKE_CXX_COMPILER=g++
cmake --build build/cpp
```

Do not add committed Visual Studio solution or project files as the canonical
build path.
