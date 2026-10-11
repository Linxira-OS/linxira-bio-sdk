# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-04/three-a.sh
# Role: one stage of the three-backend N=3 NAS window (fast-capability pass)
# Windows: 2026-10-04 (first real execution of the R backend)
# Direction: exploratory measurement, superseded by the N=10 window
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
export LC_ALL=C
export PATH="$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1
BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
W=/tmp/lxthree
mkdir -p "$W"

echo "=== generate deterministic upscaled inputs (seed 42, pure python) ==="
python3 - <<'PY'
import random, pathlib
random.seed(42)
W = pathlib.Path('/tmp/lxthree')

# 1) expression matrix: gene_id + 4 groups x 10 samples, 2000 genes
samples = [f"{g}_{i}" for g in ("control","treated","resistant","recovered") for i in range(1,11)]
with (W/'matrix.tsv').open('w') as fh:
    fh.write("gene_id\t" + "\t".join(samples) + "\n")
    for g in range(1, 2001):
        base = {0: 20, 1: 80, 2: 40, 3: 120}[g % 4]
        vals = [max(0, int(random.gauss(base, base*0.15))) for _ in range(len(samples))]
        fh.write(f"gene_{g:05d}\t" + "\t".join(map(str, vals)) + "\n")

# 2) venn table: 4 sets x 20000 identifiers with controlled overlaps
names = ["control", "treated", "validated", "literature"]
with (W/'sets.tsv').open('w') as fh:
    fh.write("\t".join(names) + "\n")
    for i in range(20000):
        row = []
        row.append(f"g{i:06d}")                      # control: all
        if i % 2 == 0: row.append(f"g{i:06d}")       # treated: half
        if i % 3 == 0: row.append(f"g{i:06d}")       # validated: third
        if i % 5 == 0: row.append(f"g{i:06d}")       # literature: fifth
        padded = row + [""] * (4 - len(row))
        fh.write("\t".join(padded) + "\n")

# 3) PDB: 3000 residues x 5 atoms, AlphaFold-style B-factors
aa = ["ALA","GLY","SER","LEU","VAL","THR","LYS","GLU"]
with (W/'structure.pdb').open('w') as fh:
    fh.write("HEADER    SYNTHETIC TEST STRUCTURE\n")
    serial = 1
    for res in range(1, 3001):
        chain = "ABCD"[res // 1000]
        name = aa[res % len(aa)]
        plddt = 30.0 + (res % 70)
        if res % 37 == 0:
            fh.write(f"TER   {serial:5d}      {name} {chain}{res:4d}\n")
            serial += 1
            continue
        for atom, elem in (("N","N"),("CA","C"),("C","C"),("O","O"),("CB","C")):
            fh.write(f"ATOM  {serial:5d}  {atom:<3s} {name:>3s} {chain}{res:4d}    "
                     f"{1.0+res*0.01:8.3f}{2.0+serial*0.001:8.3f}{3.0:8.3f}  1.00{plddt:6.2f}          {elem:>1s}\n")
            serial += 1
    fh.write("END\n")

# 4) VCF: 200000 variants over 5 contigs, SNP + indel mix
bases = "ACGT"
with (W/'variants.vcf').open('w') as fh:
    fh.write("##fileformat=VCFv4.3\n")
    for i in range(1, 6):
        fh.write(f"##contig=<ID=chr{i},length=50000000>\n")
    fh.write("#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample1\n")
    for n in range(200000):
        chrom = f"chr{1 + n % 5}"
        pos = 100 + (n * 7) % 49999900
        ref = bases[n % 4]
        alt = bases[(n + 1) % 4]
        if n % 11 == 0:
            ref, alt = ref, ref + alt          # insertion
        elif n % 13 == 0:
            ref, alt = ref + alt, ref          # deletion
        qual = 10 + (n * 13) % 90
        fh.write(f"{chrom}\t{pos}\t.\t{ref}\t{alt}\t{qual}\tPASS\tDP={(n % 120) + 1}\tGT:DP\t0/1:{(n % 120) + 1}\n")

# 5) enrichment: query gene list + association table (300 terms x 150 genes)
with (W/'genes.txt').open('w') as fh:
    fh.write("gene_id\n")
    for i in range(5000):
        fh.write(f"g{i:06d}\n")
with (W/'associations.tsv').open('w') as fh:
    fh.write("gene_id\tterm_id\tterm_name\tnamespace\n")
    for t in range(300):
        for k in range(150):
            g = (t * 17 + k * 31) % 5000
            fh.write(f"g{g:06d}\tGO:{t:07d}\tTerm {t}\tgo\n")
print("generated")
PY
ls -lh "$W" | tail -6

show() {
python3 -c "
import json,sys
d=json.load(sys.stdin)['result']
print('  verdict', d['consistency'], '| speedup', d.get('speedup'), '| memory_saving', d.get('memory_saving'))
for b in d['backends']:
    print(f\"    {b['backend']:7} median {b['median_wall_ms']:11.3f} ms  iqr {b['iqr_wall_ms']:9.3f}  rss {b.get('median_peak_rss_mb')} MB\")
if d.get('findings'): print('    findings:', json.dumps(d['findings'][:2], ensure_ascii=False)[:260])
"
}

run() {
  cap=$1; shift
  cls=$1; shift
  echo "=== $cap (class $cls) ==="
  $BIN benchmark run "$cap" "$@" --backends rust,python,r --repeat 3 \
    --output "$W/out-$cap" --dataset-class "$cls" --json 2>/dev/null | show
}

run expression.pca.v1 matrix "matrix=$W/matrix.tsv"
run set.venn.v1 other "table=$W/sets.tsv"
run structure.pdb.summary.v1 structure "pdb=$W/structure.pdb"
run variant.stats.v1 other "vcf=$W/variants.vcf"
run enrichment.overrepresentation.v1 annotation "genes=$W/genes.txt" "associations=$W/associations.tsv"

echo "=== write-back ==="
python3 scripts/benchmark/analysis/update-runtime-preferences.py $W/out-*/*.benchmark.json
git --no-pager diff --stat runtime-preferences.json
echo THREE-A-DONE
