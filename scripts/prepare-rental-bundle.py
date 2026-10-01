#!/usr/bin/env python3
"""Assemble the cloud-rental transfer bundle (FTP / JupyterLab upload).

Downloads the AutoDock Vina official basic-docking example files once,
verifies them by size and SHA-256, stages a runbook, and produces one
tar.gz plus SHA256SUMS.txt under release-artifacts-rental/ (git-ignored).

Usage:
    python scripts/prepare-rental-bundle.py            # build/refresh bundle
    python scripts/prepare-rental-bundle.py --check    # verify existing files only
"""

from __future__ import annotations

import argparse
import hashlib
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STAGING = ROOT / "release-artifacts-rental" / "linxira-rental-bundle"
BASE_URL = (
    "https://raw.githubusercontent.com/ccsb-scripps/AutoDock-Vina/develop/"
    "example/basic_docking"
)

# (relative path in bundle, source file name, expected bytes, sha256 filled on
# first download and pinned afterwards)
# (bundle path, upstream subdir + name, expected size, local seed dir)
FILES: list[tuple[str, str, int | None, str | None]] = [
    ("docking/1iep_receptor.pdbqt", "solution/1iep_receptor.pdbqt", 216160, None),
    ("docking/1iep_ligand.pdbqt", "solution/1iep_ligand.pdbqt", 3841, None),
    ("docking/1iep_receptor.box.txt", "solution/1iep_receptor.box.txt", 101, None),
    ("docking/expected/1iep_ligand_vina_out.pdbqt", "solution/1iep_ligand_vina_out.pdbqt", 16240, None),
    ("docking/raw/1iep_ligand.sdf", "data/1iep_ligand.sdf", 6621, None),
    ("docking/raw/1iep_receptorH.pdb", "data/1iep_receptorH.pdb", 357444, None),
]
# Files already fetched by hand land in TEMP; seed from there before hitting
# the network again (raw.githubusercontent occasionally 502s).
SEED_DIRS = [Path("/tmp"), Path("C:/tmp"), Path(__import__("tempfile").gettempdir())]

RUNBOOK = """\
# Linxira cloud-rental runbook (2026-10-02 round 1)

Machines: vGPU-32GB (RTX 4080S-based, sm_89) + Moore Threads MTT S4000
(MUSA); V100-32GB (sm_70) / Tesla T4 (sm_75) as optional third/edge
points. cutile-rs runs on the vGPU-32GB this round (first verify the
host driver supports CUDA 13.2+).

## T0 — setup (<= 15 min)

    git clone https://github.com/Linxira-OS/linxira-bio-sdk.git
    cd linxira-bio-sdk
    cargo build --release -p linxira-bio-cli          # in engine/
    ./target/release/linxira-bio doctor --json        # sanity
    ./target/release/linxira-bio environment audit --json > audit-$(hostname).json

## T1 — molecular docking (PRIORITY, <= 30 min)

Install the native tool (never bundled; probed by name):

    mamba install -y -c conda-forge autodock-vina
    vina --version

Dock the official 1iep example through the capability:

    BIN=target/release/linxira-bio
    $BIN chemistry dock docking/1iep_receptor.pdbqt docking/1iep_ligand.pdbqt out.pdbqt \\
      --center-x 15.190 --center-y 53.903 --center-z 16.917 \\
      --size-x 20 --size-y 20 --size-z 20 --seed 42 --json > dock.json
    cat dock.json

Acceptance:
  * exit 0; dock.json status=ok; docking.modes length >= 3
  * best_affinity_kcal_per_mol within [-14.5, -12.0]
    (official expected value: -13.234, see docking/expected/)
  * reproducibility: rerun with the same seed -> identical best affinity
    and mode count (record both JSON files)

Optional deeper check (Meeko prep route): convert raw/1iep_ligand.sdf with
mk_prepare_ligand.py and confirm the docked affinity matches the pdbqt route.

## T2 — wgpu baseline on the rental GPU (<= 30 min)

    cd gpu-lab
    cargo build --release
    ./target/release/gpu-lab probe
    ./target/release/gpu-lab gpu-bench > gpu-bench-<card>.json

Acceptance:
  * probe lists the rental GPU (adapter name + backend recorded)
  * gpu-bench: every kernel pass=true (histogram integer-exact, pearson
    delta <= 1e-5); keep the JSON for the ledger
  * risk acknowledged: datacenter Tesla drivers may not expose Vulkan;
    if probe finds no non-CPU adapter, record that as the finding

## T3 — vendor stacks (<= 40 min)

NVIDIA vGPU-32GB (sm_89):
    nvidia-smi   # confirm driver; CUDA 13.2+ needed by cutile-rs
    cargo build --features gpu-cutile   # in gpu-lab
    If the driver qualifies: run the pearson v3 kernel through cutile and
    record phased timings next to the wgpu numbers. Any failure is a
    documented outcome (driver cap, vGPU compute policy) - record verbatim.

V100 / T4 (if rented as edge points):
    EXPECTED: cutile-rs declares sm_80+; sm_70/75 results are boundary
    records, not plan failures. Record the exact error text.

Moore S4000:
    Check musa-smi / mthreads-gmi driver version (listing showed 2.7.0;
    confirm on the machine).
    cudarc-musa is not yet on crates.io (checked 2026-10-01); attempt a git
    dependency build in a scratch crate and record the outcome. Also run T2
    here: S4000 has an official Vulkan 1.3 driver, so wgpu may enumerate —
    that is the primary S4000 datapoint for our fallback layer.

## T4 — SSR three-backend parity (<= 20 min)

    mamba install -y -c conda-forge pytrf
    BIN=target/release/linxira-bio
    $BIN sequence ssr tests/fixtures/sequences/tiny.fa ssr_rust.tsv --json
    $BIN sequence ssr tests/fixtures/sequences/tiny.fa ssr_py.tsv --backend python --json
    # acceptance: the two JSON summaries are identical (records + summary)
    # larger input: simulate sequence big.fa --count 1 --length 1000000 --seed 7

## Artifacts to bring home (scp / FTP)

    audit-*.json, dock.json, out.pdbqt, gpu-bench-*.json, ssr_*.tsv,
    ssr_*.json, probe output, vendor-stack logs, all with the machine's
    nvidia-smi / musa-smi snapshot.
"""

EXPECTED_BEST_AFFINITY = -13.234


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fetch(relative: str, source: str, expected_size: int | None) -> Path:
    target = STAGING / relative
    if target.is_file() and (expected_size is None or target.stat().st_size == expected_size):
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    name = source.split("/", 1)[1]
    for seed_dir in SEED_DIRS:
        candidate = seed_dir / name
        if candidate.is_file() and (expected_size is None or candidate.stat().st_size == expected_size):
            print(f"seeding {relative} from {candidate}")
            target.write_bytes(candidate.read_bytes())
            return target
    url = f"{BASE_URL}/{source}"
    print(f"fetching {url}")
    request = urllib.request.Request(url, headers={"User-Agent": "linxira-rental-bundle"})
    with urllib.request.urlopen(request, timeout=60) as response, target.open("wb") as handle:
        handle.write(response.read())
    if expected_size is not None and target.stat().st_size != expected_size:
        raise SystemExit(f"size mismatch for {relative}: {target.stat().st_size} != {expected_size}")
    return target


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()

    staged: list[Path] = []
    for relative, source, expected_size, _ in FILES:
        path = STAGING / relative
        if arguments.check:
            if not path.is_file():
                print(f"missing: {relative}")
                return 1
        else:
            path = fetch(relative, source, expected_size)
        staged.append(path)

    (STAGING / "RUNBOOK.md").write_text(RUNBOOK, encoding="utf-8", newline="\n")
    staged.append(STAGING / "RUNBOOK.md")

    sums = STAGING / "MANIFEST.SHA256"
    sums.write_text(
        "".join(f"{sha256_file(path)}  {path.relative_to(STAGING).as_posix()}\n" for path in sorted(staged)),
        encoding="utf-8",
        newline="\n",
    )

    if arguments.check:
        print("bundle files verified")
        return 0

    archive = STAGING.parent / "linxira-rental-bundle-2026-10-02.tar.gz"
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(STAGING, arcname="linxira-rental-bundle")
    sums_archive = archive.parent / "SHA256SUMS.txt"
    sums_archive.write_text(f"{sha256_file(archive)}  {archive.name}\n", encoding="utf-8", newline="\n")
    print(f"bundle:  {archive}")
    print(f"sums:    {sums_archive}")
    print(f"expected best affinity for acceptance: {EXPECTED_BEST_AFFINITY} kcal/mol")
    return 0


if __name__ == "__main__":
    sys.exit(main())
