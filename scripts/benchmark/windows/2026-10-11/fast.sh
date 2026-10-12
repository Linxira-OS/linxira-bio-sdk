# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-11/fast.sh
# Role: one N=10 NAS stage (six fast capabilities)
# Windows: 2026-10-11
# Direction: the archived N=10 three-backend measurement
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
export LC_ALL=C
export PATH="$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1
BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
W=/tmp/lxn10
mkdir -p "$W"

# Measurement hygiene: wait out any residual load from the aborted run.
for attempt in $(seq 1 24); do
  load=$(cut -d' ' -f1 /proc/loadavg)
  case "$load" in 0.*|1.0*) echo "load settled at $load"; break;; esac
  echo "waiting for load to settle (now $load)"; sleep 5
done
uptime

# Lock/environment gate (added 2026-10-12): the measurement is only reproducible if the
# interpreter environment is the one the pack lock describes. Numbers produced under a
# different environment must not be presented as reproducible.
python3 scripts/benchmark/analysis/verify-lock-env.py \
  --lock workflows/org.linxira.benchmark-python/requirements.lock || {
    echo "refusing to measure: interpreter environment differs from requirements.lock" >&2
    exit 1
  }

# Inputs are regenerated only when missing (same seed/parameters as the N=3 window).
python3 - <<'PY'
import pathlib, random
W = pathlib.Path('/tmp/lxn10')
need = [W/'matrix.tsv', W/'sets.tsv', W/'structure.pdb', W/'variants.vcf', W/'genes.txt', W/'associations.tsv']
if all(p.is_file() for p in need):
    print("inputs already present")
    raise SystemExit
random.seed(42)
samples = [f"{g}_{i}" for g in ("control","treated","resistant","recovered") for i in range(1,11)]
with (W/'matrix.tsv').open('w') as fh:
    fh.write("gene_id\t" + "\t".join(samples) + "\n")
    for g in range(1, 2001):
        base = {0: 20, 1: 80, 2: 40, 3: 120}[g % 4]
        fh.write(f"gene_{g:05d}\t" + "\t".join(str(max(0, int(random.gauss(base, base*0.15)))) for _ in samples) + "\n")
with (W/'sets.tsv').open('w') as fh:
    fh.write("control\ttreated\tvalidated\tliterature\n")
    for i in range(20000):
        row = [f"g{i:06d}"] + ([f"g{i:06d}"] if i % 2 == 0 else [""]) + ([f"g{i:06d}"] if i % 3 == 0 else [""]) + ([f"g{i:06d}"] if i % 5 == 0 else [""])
        fh.write("\t".join(row) + "\n")
aa = ["ALA","GLY","SER","LEU","VAL","THR","LYS","GLU"]
with (W/'structure.pdb').open('w') as fh:
    fh.write("HEADER    SYNTHETIC TEST STRUCTURE\n")
    serial = 1
    for res in range(1, 3001):
        chain = "ABCD"[res // 1000]; name = aa[res % len(aa)]; plddt = 30.0 + (res % 70)
        if res % 37 == 0:
            fh.write(f"TER   {serial:5d}      {name} {chain}{res:4d}\n"); serial += 1; continue
        for atom, elem in (("N","N"),("CA","C"),("C","C"),("O","O"),("CB","C")):
            fh.write(f"ATOM  {serial:5d}  {atom:<3s} {name:>3s} {chain}{res:4d}    {1.0+res*0.01:8.3f}{2.0+serial*0.001:8.3f}{3.0:8.3f}  1.00{plddt:6.2f}          {elem:>1s}\n")
            serial += 1
    fh.write("END\n")
bases = "ACGT"
with (W/'variants.vcf').open('w') as fh:
    fh.write("##fileformat=VCFv4.3\n")
    for i in range(1, 6): fh.write(f"##contig=<ID=chr{i},length=50000000>\n")
    fh.write("#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample1\n")
    for n in range(200000):
        ref = bases[n % 4]; alt = bases[(n + 1) % 4]
        if n % 11 == 0: alt = ref + alt
        elif n % 13 == 0: ref, alt = ref + alt, ref
        fh.write(f"chr{1 + n % 5}\t{100 + (n * 7) % 49999900}\t.\t{ref}\t{alt}\t{10 + (n * 13) % 90}\tPASS\tDP={(n % 120) + 1}\tGT:DP\t0/1:{(n % 120) + 1}\n")
with (W/'genes.txt').open('w') as fh:
    fh.write("gene_id\n")
    for i in range(5000): fh.write(f"g{i:06d}\n")
with (W/'associations.tsv').open('w') as fh:
    fh.write("gene_id\tterm_id\tterm_name\tnamespace\n")
    for t in range(300):
        for k in range(150):
            fh.write(f"g{(t * 17 + k * 31) % 5000:06d}\tGO:{t:07d}\tTerm {t}\tgo\n")
print("inputs regenerated")
PY

show() {
python3 -c "
import json,sys
d=json.load(sys.stdin)['result']
print('  verdict', d['consistency'], '| precision', d['environment'].get('precision'), '| speedup', d.get('speedup'))
for b in d['backends']:
    runs=[r['wall_ms'] for r in b.get('runs',[]) if r.get('ok')]
    print(f\"    {b['backend']:7} median {b['median_wall_ms']:11.3f} ms  iqr {b['iqr_wall_ms']:9.3f}  runs {len(runs)}  rss {b.get('median_peak_rss_mb')} MB\")
if d.get('findings'): print('    findings:', json.dumps(d['findings'][:2], ensure_ascii=False)[:200])
"
}

run() { cap=$1; shift; cls=$1; shift; echo "=== $cap N=10 (class $cls) ==="; \
  $BIN benchmark run "$cap" "$@" --backends rust,python,r --repeat 10 \
    --output "$W/out-$cap" --dataset-class "$cls" --json 2>/dev/null | show; }

date -u +"start %H:%M:%SZ"
run sequence.stats.v1 sequence "fasta=/tmp/lxg/big.fa"
run expression.pca.v1 matrix "matrix=$W/matrix.tsv"
run set.venn.v1 other "table=$W/sets.tsv"
run enrichment.overrepresentation.v1 annotation "genes=$W/genes.txt" "associations=$W/associations.tsv"
run structure.pdb.summary.v1 structure "pdb=$W/structure.pdb"
run variant.stats.v1 other "vcf=$W/variants.vcf"
date -u +"end %H:%M:%SZ"
echo N10-FAST-DONE
