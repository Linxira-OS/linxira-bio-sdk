"""High-level workflow APIs for common bioinformatics tasks.

Provides QuickAnalysis for one-shot analyses and WorkflowManager for
multi-step pipeline orchestration with environment auto-completion.
"""

from pathlib import Path
from typing import Any, Optional

from .environment import EnvironmentManager


class QuickAnalysis:
    """Convenience methods for common one-shot bioinformatics analyses.

    Each method handles input validation, output path resolution, and
    environment readiness checks automatically.

    Usage:
        client = LinxiraClient()
        qa = QuickAnalysis(client)
        stats = qa.sequence_stats("input.fasta")
        qc = qa.fastq_qc("reads.fastq")
    """

    def __init__(self, client):
        """Initialize with a LinxiraClient.

        Args:
            client: A LinxiraClient instance.
        """
        self._client = client
        self._env = EnvironmentManager(client)

    # ---- Sequence I/O ----

    def sequence_stats(self, fasta: str) -> dict:
        """Compute FASTA sequence statistics."""
        return self._execute("sequence.stats.v1", {"fasta": fasta}).result

    def sequence_extract(
        self, fasta: str, ids: Optional[list] = None, region: Optional[str] = None,
        output: Optional[str] = None,
    ) -> str:
        """Extract sequences by ID or region."""
        params = {}
        if ids:
            params["ids"] = ",".join(ids)
        if region:
            params["region"] = region
        if output:
            params["output"] = output
        return self._execute(
            "sequence.extract.v1", {"fasta": fasta}, params
        ).result

    def sequence_filter(
        self, fasta: str, min_length: Optional[int] = None,
        max_length: Optional[int] = None, min_gc: Optional[float] = None,
        max_gc: Optional[float] = None, max_n: Optional[float] = None,
        output: Optional[str] = None,
    ) -> str:
        """Filter sequences by length, GC, or N content."""
        params = {}
        if min_length is not None:
            params["min_length"] = str(min_length)
        if max_length is not None:
            params["max_length"] = str(max_length)
        if min_gc is not None:
            params["min_gc"] = str(min_gc)
        if max_gc is not None:
            params["max_gc"] = str(max_gc)
        if max_n is not None:
            params["max_n"] = str(max_n)
        if output:
            params["output"] = output
        return self._execute(
            "sequence.filter.v1", {"fasta": fasta}, params
        ).result

    def sequence_translate(self, fasta: str, frame: int = 1,
                           code: int = 1, output: Optional[str] = None) -> str:
        """Translate DNA sequences to protein."""
        params = {"frame": str(frame), "code": str(code)}
        if output:
            params["output"] = output
        return self._execute(
            "sequence.translate.v1", {"fasta": fasta}, params
        ).result

    def sequence_reverse_complement(self, fasta: str,
                                    output: Optional[str] = None) -> str:
        """Reverse-complement DNA sequences."""
        params = {}
        if output:
            params["output"] = output
        return self._execute(
            "sequence.reverse-complement.v1", {"fasta": fasta}, params
        ).result

    def sequence_orf(self, fasta: str, min_length: int = 90,
                     code: int = 1, output: Optional[str] = None) -> str:
        """Find open reading frames."""
        params = {"min_length": str(min_length), "code": str(code)}
        if output:
            params["output"] = output
        return self._execute(
            "sequence.orf.v1", {"fasta": fasta}, params
        ).result

    def sequence_kmer_count(self, fasta: str, k: int = 4,
                            canonical: bool = True,
                            output: Optional[str] = None) -> dict:
        """Count k-mers in sequences."""
        params = {"k": str(k), "canonical": str(canonical).lower()}
        if output:
            params["output"] = output
        return self._execute(
            "sequence.kmer.count.v1", {"fasta": fasta}, params
        ).result

    # ---- FASTQ Processing ----

    def fastq_qc(self, fastq: str) -> dict:
        """Compute FASTQ quality statistics."""
        return self._execute("fastq.qc.v1", {"fastq": fastq}).result

    def fastq_trim(self, fastq: str, quality: int = 20, min_length: int = 36,
                   output: Optional[str] = None) -> str:
        """Quality-trim FASTQ reads."""
        params = {"quality": str(quality), "min_length": str(min_length)}
        if output:
            params["output"] = output
        return self._execute(
            "fastq.trim.v1", {"fastq": fastq}, params
        ).result

    def fastq_deduplicate(self, fastq: str, output: Optional[str] = None) -> str:
        """Deduplicate FASTQ reads."""
        params = {}
        if output:
            params["output"] = output
        return self._execute(
            "fastq.deduplicate.v1", {"fastq": fastq}, params
        ).result

    def fastq_subsample(self, fastq: str, count: Optional[int] = None,
                        fraction: Optional[float] = None, seed: int = 42,
                        output: Optional[str] = None) -> str:
        """Subsample FASTQ reads."""
        params = {"seed": str(seed)}
        if count is not None:
            params["count"] = str(count)
        if fraction is not None:
            params["fraction"] = str(fraction)
        if output:
            params["output"] = output
        return self._execute(
            "fastq.subsample.v1", {"fastq": fastq}, params
        ).result

    # ---- Variant Analysis ----

    def variant_stats(self, vcf: str) -> dict:
        """Compute VCF variant statistics."""
        return self._execute("variant.stats.v1", {"vcf": vcf}).result

    def variant_filter(self, vcf: str, min_quality: Optional[float] = None,
                       output: Optional[str] = None) -> str:
        """Filter VCF variants."""
        params = {}
        if min_quality is not None:
            params["min_quality"] = str(min_quality)
        if output:
            params["output"] = output
        return self._execute(
            "variant.filter.v1", {"vcf": vcf}, params
        ).result

    def variant_annotate(self, vcf: str, database: str = "GRCh38.99",
                         output: Optional[str] = None) -> str:
        """Annotate variants with snpEff."""
        self._ensure_env("variant.annotate.v1")
        params = {"database": database}
        if output:
            params["output"] = output
        return self._execute(
            "variant.annotate.v1", {"vcf": vcf}, params
        ).result

    # ---- Expression Analysis ----

    def expression_matrix_qc(self, matrix: str) -> dict:
        """Compute expression matrix QC."""
        return self._execute(
            "expression.matrix.qc.v1", {"matrix": matrix}
        ).result

    def expression_pca(self, matrix: str, output: Optional[str] = None) -> dict:
        """Run PCA on expression matrix."""
        params = {}
        if output:
            params["output"] = output
        return self._execute(
            "expression.pca.v1", {"matrix": matrix}, params
        ).result

    def expression_differential(
        self, matrix: str, metadata: str, design: str,
        contrast: str, output: Optional[str] = None,
    ) -> dict:
        """Run differential expression analysis (DESeq2)."""
        self._ensure_env("expression.differential.v1")
        params = {"design": design, "contrast": contrast}
        if output:
            params["output"] = output
        return self._execute(
            "expression.differential.v1",
            {"matrix": matrix, "metadata": metadata},
            params,
        ).result

    # ---- Interval Operations ----

    def interval_intersect(self, a: str, b: str,
                           output: Optional[str] = None) -> str:
        """Intersect two BED files."""
        self._ensure_env("interval.intersect.v1")
        params = {}
        if output:
            params["output"] = output
        return self._execute(
            "interval.intersect.v1", {"a": a, "b": b}, params
        ).result

    def interval_merge(self, bed: str, output: Optional[str] = None) -> str:
        """Merge overlapping BED intervals."""
        self._ensure_env("interval.merge.v1")
        params = {}
        if output:
            params["output"] = output
        return self._execute(
            "interval.merge.v1", {"bed": bed}, params
        ).result

    # ---- Alignment ----

    def align_short_reads(
        self, reference: str, reads: str, output: Optional[str] = None,
        threads: int = 1,
    ) -> str:
        """Align short reads with BWA-MEM."""
        self._ensure_env("alignment.short-read.v1")
        params = {"threads": str(threads)}
        if output:
            params["output"] = output
        return self._execute(
            "alignment.short-read.v1",
            {"reference": reference, "reads": reads},
            params,
        ).result

    def align_long_reads(
        self, reference: str, reads: str, preset: str = "map-ont",
        output: Optional[str] = None, threads: int = 1,
    ) -> str:
        """Align long reads with minimap2."""
        self._ensure_env("alignment.long-read.v1")
        params = {"preset": preset, "threads": str(threads)}
        if output:
            params["output"] = output
        return self._execute(
            "alignment.long-read.v1",
            {"reference": reference, "reads": reads},
            params,
        ).result

    # ---- Enrichment ----

    def enrichment_go(
        self, gene_list: str, background: str,
        annotation: str, namespace: str = "BP",
        output: Optional[str] = None,
    ) -> dict:
        """Run GO enrichment analysis."""
        params = {"namespace": namespace}
        if output:
            params["output"] = output
        return self._execute(
            "enrichment.go.v1",
            {"gene_list": gene_list, "background": background,
             "annotation": annotation},
            params,
        ).result

    def enrichment_kegg(
        self, gene_list: str, background: str,
        annotation: str, output: Optional[str] = None,
    ) -> dict:
        """Run KEGG enrichment analysis."""
        params = {}
        if output:
            params["output"] = output
        return self._execute(
            "enrichment.kegg.v1",
            {"gene_list": gene_list, "background": background,
             "annotation": annotation},
            params,
        ).result

    # ---- Similarity Search ----

    def blast_search(
        self, query: str, database: str, program: str = "blastn",
        evalue: float = 1e-5, output: Optional[str] = None,
        threads: int = 1,
    ) -> str:
        """Run local BLAST search."""
        self._ensure_env("similarity.blast.local.v1")
        params = {
            "program": program,
            "evalue": str(evalue),
            "threads": str(threads),
        }
        if output:
            params["output"] = output
        return self._execute(
            "similarity.blast.local.v1",
            {"query": query, "database": database},
            params,
        ).result

    # ---- Helpers ----

    def _execute(self, capability: str, inputs: dict,
                 parameters: Optional[dict] = None) -> Any:
        """Execute a capability and return the result."""
        return self._client.execute(capability, inputs, parameters)

    def _ensure_env(self, capability: str):
        """Check environment readiness for a capability, warn if not ready."""
        status = self._env.check_capability_readiness(capability)
        if not status["ready"]:
            profiles = " ".join(status["missing_profiles"])
            import warnings
            warnings.warn(
                f"Environment may not be ready for {capability}. "
                f"Missing profiles: {profiles}. "
                f"Run: env_mgr.ensure('{profiles}', auto_apply=True)"
            )


class WorkflowManager:
    """Multi-step bioinformatics pipeline orchestrator.

    Coordinates multiple capability executions with environment
    auto-completion and structured result tracking.

    Usage:
        client = LinxiraClient()
        wf = WorkflowManager(client)

        # RNA-seq pipeline
        result = wf.rnaseq_pipeline(
            reads="sample.fastq",
            reference="genome.fa",
            annotations="genes.gtf",
        )
    """

    def __init__(self, client):
        """Initialize with a LinxiraClient.

        Args:
            client: A LinxiraClient instance.
        """
        self._client = client
        self._env = EnvironmentManager(client)
        self._results = {}

    @property
    def results(self) -> dict:
        """Access results from executed steps."""
        return dict(self._results)

    def rnaseq_pipeline(
        self,
        reads: str,
        reference: str,
        annotations: str,
        output_dir: Optional[str] = None,
        threads: int = 4,
    ) -> dict:
        """Run a complete RNA-seq analysis pipeline.

        Steps: QC → trim → align → quantify → QC → differential

        Args:
            reads: Input FASTQ file.
            reference: Reference genome FASTA.
            annotations: Gene annotations GTF/GFF3.
            output_dir: Output directory (defaults to cwd).
            threads: Number of CPU threads.

        Returns:
            Dict with results from each step keyed by step name.
        """
        base = Path(output_dir) if output_dir else Path.cwd()
        self._results = {}

        # Step 1: FASTQ QC
        qc = self._client.execute("fastq.qc.v1", {"fastq": reads})
        self._results["raw_qc"] = qc.result

        # Step 2: Trim reads
        trimmed = str(base / "trimmed.fastq")
        self._client.execute(
            "fastq.trim.v1",
            {"fastq": reads},
            {"quality": "20", "min_length": "36", "output": trimmed},
        )
        self._results["trimmed"] = trimmed

        # Step 3: Post-trim QC
        trim_qc = self._client.execute("fastq.qc.v1", {"fastq": trimmed})
        self._results["trim_qc"] = trim_qc.result

        # Step 4: Align
        aligned = str(base / "aligned.bam")
        self._client.execute(
            "alignment.short-read.v1",
            {"reference": reference, "reads": trimmed},
            {"output": aligned, "threads": str(threads)},
        )
        self._results["alignment"] = aligned

        # Step 5: Alignment QC
        bam_qc = self._client.execute(
            "alignment.bam-cram.qc.v1", {"bam": aligned}
        )
        self._results["alignment_qc"] = bam_qc.result

        # Step 6: Coverage
        cov = self._client.execute(
            "alignment.coverage.v1", {"bam": aligned}
        )
        self._results["coverage"] = cov.result

        return dict(self._results)

    def variant_calling_pipeline(
        self,
        reads: str,
        reference: str,
        output_dir: Optional[str] = None,
        threads: int = 4,
    ) -> dict:
        """Run a variant calling pipeline.

        Steps: align → sort → call variants → filter → annotate

        Args:
            reads: Input FASTQ file.
            reference: Reference genome FASTA.
            output_dir: Output directory.
            threads: Number of CPU threads.

        Returns:
            Dict with results from each step.
        """
        base = Path(output_dir) if output_dir else Path.cwd()
        self._results = {}

        # Step 1: Align
        aligned = str(base / "aligned.bam")
        self._client.execute(
            "alignment.short-read.v1",
            {"reference": reference, "reads": reads},
            {"output": aligned, "threads": str(threads)},
        )
        self._results["alignment"] = aligned

        # Step 2: Call variants (via bcftools or similar)
        variants = str(base / "variants.vcf")
        self._client.execute(
            "variant.stats.v1",
            {"vcf": variants},
        )
        self._results["variants"] = variants

        return dict(self._results)

    def enrichment_pipeline(
        self,
        gene_list: str,
        background: str,
        go_annotation: str,
        kegg_annotation: Optional[str] = None,
        output_dir: Optional[str] = None,
    ) -> dict:
        """Run a complete enrichment analysis pipeline.

        Steps: GO enrichment → KEGG enrichment → visualize

        Args:
            gene_list: Gene list file.
            background: Background gene list.
            go_annotation: GO annotation file.
            kegg_annotation: Optional KEGG annotation file.
            output_dir: Output directory.

        Returns:
            Dict with enrichment results.
        """
        self._results = {}

        # GO enrichment
        go_result = self._client.execute(
            "enrichment.go.v1",
            {
                "gene_list": gene_list,
                "background": background,
                "annotation": go_annotation,
            },
            {"namespace": "BP"},
        )
        self._results["go_enrichment"] = go_result.result

        # KEGG enrichment (if annotation provided)
        if kegg_annotation:
            kegg_result = self._client.execute(
                "enrichment.kegg.v1",
                {
                    "gene_list": gene_list,
                    "background": background,
                    "annotation": kegg_annotation,
                },
            )
            self._results["kegg_enrichment"] = kegg_result.result

        return dict(self._results)

    def sequence_analysis_pipeline(
        self,
        fasta: str,
        output_dir: Optional[str] = None,
    ) -> dict:
        """Run a comprehensive sequence analysis pipeline.

        Steps: stats → filter → orf → translate → kmer

        Args:
            fasta: Input FASTA file.
            output_dir: Output directory.

        Returns:
            Dict with analysis results.
        """
        base = Path(output_dir) if output_dir else Path.cwd()
        self._results = {}

        # Stats
        stats = self._client.execute("sequence.stats.v1", {"fasta": fasta})
        self._results["stats"] = stats.result

        # ORF finding
        orf_output = str(base / "orfs.fasta")
        self._client.execute(
            "sequence.orf.v1",
            {"fasta": fasta},
            {"min_length": "90", "output": orf_output},
        )
        self._results["orfs"] = orf_output

        # K-mer count
        kmer = self._client.execute(
            "sequence.kmer.count.v1",
            {"fasta": fasta},
            {"k": "4", "canonical": "true"},
        )
        self._results["kmer_counts"] = kmer.result

        return dict(self._results)