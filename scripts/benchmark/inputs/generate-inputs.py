#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/inputs/generate-inputs.py
# Role: deterministic benchmark input generator (synthetic tables, granularity FASTAs, engine-simulated FASTA/FASTQ)
# Windows: 2026-10-11 (N=10 window inputs and the granularity experiment)
# Direction: reproducibility — byte-identical inputs on any machine
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
"""Deterministic generator for every benchmark input used in the archived runs.

Reproducibility contract: the same command must yield byte-identical inputs on
any machine, which is what makes ``sha256`` comparisons in the archive
meaningful. Two families exist:

``--set synthetic``  (pure Python, ``random.seed(42)``)
    ``matrix.tsv`` (2000x40 count matrix), ``sets.tsv`` (20k x 4 identifier
    sets), ``structure.pdb`` (3000 residues / 24k atoms), ``variants.vcf``
    (200k records), ``genes.txt`` + ``associations.tsv`` (5k genes / 45k
    annotations).

``--set granularity``  (numpy, ``default_rng(42)``)
    ``g_one.fa`` (1 x 100 Mb), ``g_twenty.fa`` (20 x 5 Mb), ``g_many.fa``
    (100 000 x 1 kb) — the record-shape experiment of the 2026-10-11 window.

``--set sequence`` / ``--set reads``  (delegate to the engine's own seeded
    simulator, so the FASTA/FASTQ inputs are reproducible from the product
    binary rather than re-implemented here)
    ``big.fa`` = ``simulate sequence --count 20 --length 5000000 --seed 42``
    ``reads200.fq`` = ``simulate reads big.fa --read-length 150 --coverage 100 --seed 42``

Usage:
    python scripts/benchmark/inputs/generate-inputs.py --set synthetic --out /tmp/lxn10
    python scripts/benchmark/inputs/generate-inputs.py --set sequence --out /tmp/lxn10 \\
        --binary target/release/linxira-bio
"""

from __future__ import annotations

import argparse
import random
import subprocess
import sys
from pathlib import Path

SEED = 42
SAMPLES_PER_GROUP = 10
GENES = 2000
SET_ROWS = 20000
RESIDUES = 3000
VARIANTS = 200000
ANNOTATED_GENES = 5000
TERMS = 300
GENES_PER_TERM = 150

FASTA_RECORDS = 20
FASTA_BASES = 5_000_000
READ_LENGTH = 150
READ_COVERAGE = 100


def synthetic(out: Path) -> list[Path]:
    """Byte-for-byte the inputs used by the archived three-backend runs."""
    random.seed(SEED)
    written: list[Path] = []

    samples = [f"{group}_{index}" for group in ("control", "treated", "resistant", "recovered")
               for index in range(1, SAMPLES_PER_GROUP + 1)]
    path = out / "matrix.tsv"
    with path.open("w", encoding="utf-8", newline="") as fh:
        fh.write("gene_id\t" + "\t".join(samples) + "\n")
        for gene in range(1, GENES + 1):
            base = {0: 20, 1: 80, 2: 40, 3: 120}[gene % 4]
            fh.write(f"gene_{gene:05d}\t" + "\t".join(
                str(max(0, int(random.gauss(base, base * 0.15)))) for _ in samples) + "\n")
    written.append(path)

    path = out / "sets.tsv"
    with path.open("w", encoding="utf-8", newline="") as fh:
        fh.write("control\ttreated\tvalidated\tliterature\n")
        for index in range(SET_ROWS):
            item = f"g{index:06d}"
            row = [item] + ([item] if index % 2 == 0 else [""]) + \
                ([item] if index % 3 == 0 else [""]) + ([item] if index % 5 == 0 else [""])
            fh.write("\t".join(row) + "\n")
    written.append(path)

    residues = ["ALA", "GLY", "SER", "LEU", "VAL", "THR", "LYS", "GLU"]
    path = out / "structure.pdb"
    with path.open("w", encoding="utf-8", newline="") as fh:
        fh.write("HEADER    SYNTHETIC TEST STRUCTURE\n")
        serial = 1
        for residue in range(1, RESIDUES + 1):
            chain = "ABCD"[residue // 1000]
            name = residues[residue % len(residues)]
            plddt = 30.0 + (residue % 70)
            if residue % 37 == 0:
                fh.write(f"TER   {serial:5d}      {name} {chain}{residue:4d}\n")
                serial += 1
                continue
            for atom, element in (("N", "N"), ("CA", "C"), ("C", "C"), ("O", "O"), ("CB", "C")):
                fh.write(f"ATOM  {serial:5d}  {atom:<3s} {name:>3s} {chain}{residue:4d}    "
                         f"{1.0 + residue * 0.01:8.3f}{2.0 + serial * 0.001:8.3f}{3.0:8.3f}  "
                         f"1.00{plddt:6.2f}          {element:>1s}\n")
                serial += 1
        fh.write("END\n")
    written.append(path)

    bases = "ACGT"
    path = out / "variants.vcf"
    with path.open("w", encoding="utf-8", newline="") as fh:
        fh.write("##fileformat=VCFv4.3\n")
        for index in range(1, 6):
            fh.write(f"##contig=<ID=chr{index},length=50000000>\n")
        fh.write("#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample1\n")
        for index in range(VARIANTS):
            ref = bases[index % 4]
            alt = bases[(index + 1) % 4]
            if index % 11 == 0:
                alt = ref + alt
            elif index % 13 == 0:
                ref, alt = ref + alt, ref
            fh.write(f"chr{1 + index % 5}\t{100 + (index * 7) % 49999900}\t.\t{ref}\t{alt}\t"
                     f"{10 + (index * 13) % 90}\tPASS\tDP={(index % 120) + 1}\tGT:DP\t"
                     f"0/1:{(index % 120) + 1}\n")
    written.append(path)

    path = out / "genes.txt"
    with path.open("w", encoding="utf-8", newline="") as fh:
        fh.write("gene_id\n")
        for index in range(ANNOTATED_GENES):
            fh.write(f"g{index:06d}\n")
    written.append(path)

    path = out / "associations.tsv"
    with path.open("w", encoding="utf-8", newline="") as fh:
        fh.write("gene_id\tterm_id\tterm_name\tnamespace\n")
        for term in range(TERMS):
            for offset in range(GENES_PER_TERM):
                fh.write(f"g{(term * 17 + offset * 31) % ANNOTATED_GENES:06d}\tGO:{term:07d}\t"
                         f"Term {term}\tgo\n")
    written.append(path)
    return written


def granularity(out: Path) -> list[Path]:
    """Same ~100 Mb of FASTA in three record shapes (seed 42, numpy generator)."""
    try:
        import numpy as np
    except ModuleNotFoundError:  # pragma: no cover - dependency hint only
        raise SystemExit("--set granularity needs numpy (pip install numpy)")

    rng = np.random.default_rng(SEED)
    letters = np.array(list("ACGT"))
    written: list[Path] = []
    for name, records, bases_each in (("g_one.fa", 1, 100_000_000),
                                      ("g_twenty.fa", 20, 5_000_000),
                                      ("g_many.fa", 100_000, 1000)):
        sequence = rng.choice(letters, size=records * bases_each)
        path = out / name
        with path.open("w", encoding="utf-8", newline="") as fh:
            for index in range(records):
                fh.write(f">rec_{index}\n")
                chunk = "".join(sequence[index * bases_each:(index + 1) * bases_each])
                for offset in range(0, len(chunk), 60):
                    fh.write(chunk[offset:offset + 60] + "\n")
        written.append(path)
    return written


def engine(binary: str, out: Path, which: str, reference: Path | None) -> list[Path]:
    """FASTA/FASTQ inputs come from the engine's own seeded simulator."""
    if which == "sequence":
        target = out / "big.fa"
        command = [binary, "simulate", "sequence", str(target),
                   "--count", str(FASTA_RECORDS), "--length", str(FASTA_BASES),
                   "--seed", str(SEED), "--json"]
    else:
        reference = reference or (out / "big.fa")
        target = out / "reads200.fq"
        command = [binary, "simulate", "reads", str(reference), str(target),
                   "--read-length", str(READ_LENGTH), "--coverage", str(READ_COVERAGE),
                   "--seed", str(SEED), "--json"]
    print("running:", " ".join(command))
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL, cwd=Path.cwd())
    return [target]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--set", required=True,
                        choices=["synthetic", "granularity", "sequence", "reads", "all"])
    parser.add_argument("--out", required=True, type=Path, help="output directory (created)")
    parser.add_argument("--binary", default=None,
                        help="linxira-bio binary, needed by --set sequence/reads/all")
    parser.add_argument("--reference", type=Path, default=None,
                        help="reference FASTA for --set reads (default: <out>/big.fa)")
    arguments = parser.parse_args()

    arguments.out.mkdir(parents=True, exist_ok=True)
    written: list[Path] = []
    if arguments.set in ("synthetic", "all"):
        written += synthetic(arguments.out)
    if arguments.set in ("granularity", "all"):
        written += granularity(arguments.out)
    if arguments.set in ("sequence", "all"):
        if not arguments.binary:
            raise SystemExit("--set sequence needs --binary (engine-simulated input)")
        written += engine(arguments.binary, arguments.out, "sequence", None)
    if arguments.set in ("reads", "all"):
        if not arguments.binary:
            raise SystemExit("--set reads needs --binary (engine-simulated input)")
        written += engine(arguments.binary, arguments.out, "reads", arguments.reference)

    for path in written:
        print(f"wrote {path.name}: {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
