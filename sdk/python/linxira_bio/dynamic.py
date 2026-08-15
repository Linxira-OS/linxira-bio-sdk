"""Dynamic API generation from capability catalog.

Automatically exposes every available capability as a method on the client,
ensuring the SDK is always in sync with the latest catalog without code changes.
"""

import re
from typing import Any, Optional


def _capability_id_to_method_name(cap_id: str) -> str:
    """Convert a capability ID to a Python method name.

    Examples:
        sequence.stats.v1        → sequence_stats
        fastq.qc.v1              → fastq_qc
        alignment.long-read.v1   → alignment_long_read
        environment.apply.v1     → environment_apply
    """
    # Remove version suffix (.v1, .v2, etc.)
    base = re.sub(r"\.v\d+$", "", cap_id)
    # Replace dots and hyphens with underscores
    return base.replace(".", "_").replace("-", "_")


def _generate_docstring(cap: dict) -> str:
    """Generate a docstring for a capability method."""
    cap_id = cap.get("id", "")
    category = cap.get("category", "unknown")
    status = cap.get("status", "unknown")
    command = cap.get("command", "")
    input_formats = cap.get("input_formats", [])
    output_formats = cap.get("output_formats", [])

    lines = [f"Execute {cap_id} ({category}).", ""]
    if command:
        lines.append(f"    CLI: {command}")
    if input_formats:
        lines.append(f"    Input formats: {', '.join(input_formats)}")
    if output_formats:
        lines.append(f"    Output formats: {', '.join(output_formats)}")
    if status != "available":
        lines.append(f"    Status: {status}")
    lines.append("")
    lines.append("    Args:")
    lines.append("        **inputs: Input file paths keyed by role name.")
    lines.append("        **params: Optional parameters for the capability.")
    lines.append("")
    lines.append("    Returns:")
    lines.append("        LinxiraResult with the execution result.")

    return "\n".join(lines)


def _make_capability_method(cap_id: str):
    """Create a callable method for a capability."""

    def method(self, **kwargs):
        """Auto-generated method for capability."""
        inputs = {}
        params = {}

        # Separate inputs from parameters based on the capability schema
        for key, value in kwargs.items():
            if isinstance(value, str) and ("." in value or "/" in value or "\\" in value):
                inputs[key] = value
            else:
                params[key] = value

        return self._client.execute(cap_id, inputs, parameters=params if params else None)

    method.__name__ = _capability_id_to_method_name(cap_id)
    method.__qualname__ = f"DynamicAPI.{method.__name__}"
    return method


class DynamicAPI:
    """Auto-generated API that exposes every catalog capability as a method.

    Usage:
        client = LinxiraClient()
        api = DynamicAPI(client)

        # All available capabilities are auto-generated as methods:
        result = api.sequence_stats(fasta="input.fasta")
        result = api.fastq_qc(fastq="reads.fastq")
        result = api.variant_annotate(vcf="input.vcf", database="GRCh38.99")

    New capabilities added to catalog.json are automatically available
    without any SDK code changes.
    """

    def __init__(self, client):
        """Initialize with a LinxiraClient and auto-generate methods.

        Args:
            client: A LinxiraClient instance.
        """
        self._client = client
        self._capabilities = {}
        self._register_capabilities()

    def _register_capabilities(self):
        """Auto-register all available capabilities as methods."""
        from .catalog import get_catalog

        catalog = get_catalog()
        for cap in catalog:
            cap_id = cap.get("id", "")
            status = cap.get("status", "")
            if status != "available":
                continue
            if not cap.get("command"):
                continue

            method_name = _capability_id_to_method_name(cap_id)
            method = _make_capability_method(cap_id)
            method.__doc__ = _generate_docstring(cap)

            setattr(self, method_name, method)
            self._capabilities[method_name] = cap_id

    def list_methods(self) -> dict:
        """List all auto-generated methods and their capability IDs.

        Returns:
            Dict mapping method names to capability IDs.
        """
        return dict(self._capabilities)

    def __dir__(self):
        """Return all available method names for tab completion."""
        base = list(super().__dir__())
        return base + list(self._capabilities.keys())

    def __repr__(self) -> str:
        count = len(self._capabilities)
        return f"DynamicAPI({count} capabilities available)"


class AutoClient:
    """A LinxiraClient subclass that auto-exposes all capabilities as methods.

    This combines LinxiraClient with DynamicAPI for the simplest possible
    interface. Every capability in the catalog becomes a direct method.

    Usage:
        client = AutoClient()
        result = client.sequence_stats(fasta="input.fasta")
        result = client.fastq_qc(fastq="reads.fastq")
        result = client.alignment_long_read(
            reference="genome.fa", reads="reads.fastq",
            preset="map-ont", threads="4"
        )

        # Check what methods are available:
        print(client.list_methods())

        # Environment auto-completion:
        env = client.environment
        audit = env.audit()
        if not audit.is_ready:
            env.ensure("genomics-cli", auto_apply=True)
    """

    def __init__(self, worker_bin=None, base_dir=None):
        """Initialize AutoClient with worker discovery and API generation.

        Args:
            worker_bin: Optional path to linxira-bio-worker binary.
            base_dir: Optional base directory for relative paths.
        """
        from .client import LinxiraClient

        # Initialize the underlying client
        self._client = LinxiraClient(worker_bin=worker_bin, base_dir=base_dir)

        # Auto-generate capability methods
        self._capabilities = {}
        self._register_capabilities()

        # Attach environment manager
        from .environment import EnvironmentManager

        self.environment = EnvironmentManager(self._client)

        # Attach quick analysis
        from .workflow import QuickAnalysis

        self.quick = QuickAnalysis(self._client)

        # Attach workflow manager
        from .workflow import WorkflowManager

        self.workflow = WorkflowManager(self._client)

    def _register_capabilities(self):
        """Auto-register all available capabilities as methods."""
        from .catalog import get_catalog

        catalog = get_catalog()
        for cap in catalog:
            cap_id = cap.get("id", "")
            status = cap.get("status", "")
            if status != "available":
                continue
            if not cap.get("command"):
                continue

            method_name = _capability_id_to_method_name(cap_id)
            method = _make_capability_method(cap_id)
            method.__doc__ = _generate_docstring(cap)

            setattr(self, method_name, method)
            self._capabilities[method_name] = cap_id

    def list_methods(self) -> dict:
        """List all auto-generated methods and their capability IDs.

        Returns:
            Dict mapping method names to capability IDs.
        """
        return dict(self._capabilities)

    def execute(self, capability: str, inputs: dict,
                parameters: Optional[dict] = None,
                job_id: Optional[str] = None):
        """Execute a capability by ID (fallback for non-auto-generated access)."""
        return self._client.execute(capability, inputs, parameters, job_id)

    def __dir__(self):
        """Return all available method names for tab completion."""
        base = list(super().__dir__())
        return base + list(self._capabilities.keys())

    def __repr__(self) -> str:
        count = len(self._capabilities)
        return f"AutoClient({count} capabilities available)"