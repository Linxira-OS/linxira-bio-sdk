#!/usr/bin/env python3
"""Docking study analysis: heavy-atom RMSD vs official reference pose + summary TSV.

Usage: python3 analyze_dock.py <bundle_docking_dir> <study_dir>
Outputs: rmsd_vs_official.tsv, summary.tsv (written into study_dir)
"""
import glob
import json
import os
import sys

import numpy as np


def parse_pdbqt_models(path):
    """Models as (N,3) heavy-atom arrays. ENDROOT must not end a model
    (startswith("END") trap); polar hydrogens (element HD/H) are skipped."""
    models = []
    coords = []
    for line in open(path):
        if line.startswith(("ATOM", "HETATM")):
            element = line[77:80].strip() or line.rstrip().split()[-1]
            if element.startswith("H"):
                continue
            coords.append((float(line[30:38]), float(line[38:46]), float(line[46:54])))
        elif line.startswith("MODEL"):
            coords = []
        elif line.strip() in ("ENDMDL", "END"):
            if coords:
                models.append(np.array(coords))
                coords = []
    if coords:
        models.append(np.array(coords))
    return models


def in_place_rmsd(a, b):
    return float(np.sqrt(np.mean(np.sum((a - b) ** 2, axis=1))))


def main(bundle_dir, study_dir):
    official = parse_pdbqt_models(os.path.join(bundle_dir, "1iep_ligand_vina_out.pdbqt"))
    ref = official[0]
    print(f"official reference: {len(official)} models, best pose {ref.shape[0]} atoms")

    rows = ["run\theavy_atom_rmsd_vs_official_best_A"]
    for path in sorted(glob.glob(os.path.join(study_dir, "A_*.pdbqt")) + glob.glob(os.path.join(study_dir, "B_*.pdbqt"))):
        name = os.path.basename(path).replace(".pdbqt", "")
        ours = parse_pdbqt_models(path)
        if not ours:
            continue
        if ours[0].shape != ref.shape:
            rows.append(f"{name}\tATOM-COUNT-MISMATCH({ours[0].shape[0]}-vs-{ref.shape[0]})")
            continue
        rows.append(f"{name}\t{in_place_rmsd(ours[0], ref):.3f}")
    with open(os.path.join(study_dir, "rmsd_vs_official.tsv"), "w") as fh:
        fh.write("\n".join(rows) + "\n")
    print("\n".join(rows))

    # summary: every run with wall time, affinity, mode count
    out = ["phase\texhaustiveness\tseed\tcpu\twall_s\tbest_affinity\tmode_count\trmsd_best_vs_rank2"]
    for path in sorted(glob.glob(os.path.join(study_dir, "A_*.json"))):
        stem = os.path.basename(path)[:-5]  # A_e32_s43
        parts = stem.split("_")
        e, seed = parts[1][1:], parts[2][1:]
        wall = read_wall(os.path.join(study_dir, f"wall_{stem}.txt"))
        out.append(summary_row("A", e, seed, 32, wall, path))
    for path in sorted(glob.glob(os.path.join(study_dir, "B_*.json"))):
        stem = os.path.basename(path)[:-5]
        cpu = stem.split("_")[1][3:]
        wall = read_wall(os.path.join(study_dir, f"wall_{stem}.txt"))
        out.append(summary_row("B", 8, 42, cpu, wall, path))
    with open(os.path.join(study_dir, "summary.tsv"), "w") as fh:
        fh.write("\n".join(out) + "\n")
    print("\n".join(out))


def read_wall(path):
    if not os.path.exists(path):
        return "NA"
    return open(path).read().strip().split()[-2]


def summary_row(phase, e, seed, cpu, wall, json_path):
    doc = json.load(open(json_path))
    if doc.get("status") != "ok":
        return f"{phase}\t{e}\t{seed}\t{cpu}\t{wall}\tERROR\tERROR\tERROR"
    modes = doc["result"]["docking"]["modes"]
    best = modes[0]["affinity_kcal_per_mol"]
    gap = modes[1]["affinity_kcal_per_mol"] - modes[0]["affinity_kcal_per_mol"] if len(modes) > 1 else 0.0
    return f"{phase}\t{e}\t{seed}\t{cpu}\t{wall}\t{best:.3f}\t{len(modes)}\t{gap:.3f}"


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
