[CmdletBinding()]
param(
    [Alias('LlvmMingwHome')]
    [string]$GnuToolchainHome = $env:RUST_GNU_HOME
)

$ErrorActionPreference = 'Stop'

$ciPythonCandidates = @(
    (Join-Path (Get-Location).Path '.venv-ci\Scripts\python.exe'),
    (Join-Path (Get-Location).Path '.venv-ci\bin\python.exe')
)
$ciPython = $ciPythonCandidates |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
if (-not $ciPython) {
    $bootstrapCandidates = @()
    if ($env:CONDA_ROOT) {
        $bootstrapCandidates += Join-Path $env:CONDA_ROOT 'python.exe'
    }
    if ($env:CONDA_PREFIX) {
        $bootstrapCandidates += Join-Path $env:CONDA_PREFIX 'python.exe'
    }
    $bootstrapCandidates += Get-Command python -All -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty Source
    $bootstrapPython = $bootstrapCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -Unique |
        Where-Object {
            & $_ -m pip --version *> $null
            $LASTEXITCODE -eq 0
        } |
        Select-Object -First 1
    if (-not $bootstrapPython) {
        throw 'No Python interpreter with pip is available to create .venv-ci.'
    }
    & $bootstrapPython -m venv .venv-ci
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to create the isolated CI Python environment.'
    }
    $ciPython = $ciPythonCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if (-not $ciPython) {
        throw 'The isolated CI Python interpreter was not created.'
    }
}
& $ciPython -m pip install --disable-pip-version-check `
    --requirement requirements-ci.txt
if ($LASTEXITCODE -ne 0) {
    throw 'Failed to install pinned CI Python dependencies.'
}

if (-not $GnuToolchainHome) {
    $GnuToolchainHome = 'C:\Rust\msys64\ucrt64'
}
$toolchainRoot = (Resolve-Path -LiteralPath $GnuToolchainHome).Path
$linkerCandidates = @(
    (Join-Path $toolchainRoot 'bin\x86_64-w64-mingw32-clang.exe'),
    (Join-Path $toolchainRoot 'bin\gcc.exe')
)
$linker = $linkerCandidates |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
    Select-Object -First 1
if (-not $linker) {
    throw "No LLVM-MinGW or MSYS2 UCRT64 linker found under: $toolchainRoot"
}

rustc +stable-x86_64-pc-windows-gnu --version
if ($LASTEXITCODE -ne 0) {
    throw 'The stable Windows GNU Rust host is not installed.'
}

$env:PATH = "$(Join-Path $toolchainRoot 'bin');$env:PATH"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $linker
$env:CC_x86_64_pc_windows_gnu = Join-Path $toolchainRoot 'bin\gcc.exe'
$env:CXX_x86_64_pc_windows_gnu = Join-Path $toolchainRoot 'bin\g++.exe'
$env:AR_x86_64_pc_windows_gnu = Join-Path $toolchainRoot 'bin\ar.exe'
$env:PKG_CONFIG_PATH = @(
    (Join-Path $toolchainRoot 'lib\pkgconfig')
    (Join-Path $toolchainRoot 'share\pkgconfig')
) -join ';'

cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    throw 'Rust formatting check failed for the Windows GNU toolchain.'
}

cargo +stable-x86_64-pc-windows-gnu clippy --locked --workspace --all-targets -- `
    -D warnings
if ($LASTEXITCODE -ne 0) {
    throw 'Rust Clippy checks failed for the Windows GNU toolchain.'
}

cargo +stable-x86_64-pc-windows-gnu test --locked --workspace
if ($LASTEXITCODE -ne 0) {
    throw 'Rust tests failed for the Windows GNU target.'
}

cargo +stable-x86_64-pc-windows-gnu run --locked -p linxira-bio-worker -- `
    tests/fixtures/jobs/sequence-stats.json
if ($LASTEXITCODE -ne 0) {
    throw 'Worker smoke test failed for the Windows GNU target.'
}

cargo +stable-x86_64-pc-windows-gnu run --locked -p linxira-bio-cli -- `
    sequence stats tests/fixtures/sequences/tiny.fa --json
if ($LASTEXITCODE -ne 0) {
    throw 'CLI smoke test failed for the Windows GNU target.'
}

& $ciPython scripts/validate-repository.py
if ($LASTEXITCODE -ne 0) {
    throw 'Repository contract validation failed.'
}
