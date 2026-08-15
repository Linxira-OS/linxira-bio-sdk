"""Main client for executing bioinformatics capabilities."""

import json
import os
import subprocess
import tempfile
import uuid
from pathlib import Path
from typing import Any, Optional


class LinxiraResult:
    """Structured result from a capability execution."""

    def __init__(
        self,
        job_id: str,
        capability: str,
        status: str,
        result: dict,
        provenance: dict,
        warnings: list,
        raw_json: str,
    ):
        self.job_id = job_id
        self.capability = capability
        self.status = status
        self.result = result
        self.provenance = provenance
        self.warnings = warnings
        self._raw_json = raw_json

    @property
    def ok(self) -> bool:
        """True if the capability executed successfully."""
        return self.status == "ok"

    @property
    def engine_version(self) -> str:
        """The engine version that produced this result."""
        return self.provenance.get("engine_version", "")

    @property
    def execution_mode(self) -> str:
        """The execution mode used."""
        return self.provenance.get("execution_mode", "")

    def to_dict(self) -> dict:
        """Return the full result envelope as a dict."""
        return json.loads(self._raw_json)

    def __repr__(self) -> str:
        return (
            f"LinxiraResult(job_id={self.job_id!r}, capability={self.capability!r}, "
            f"status={self.status!r})"
        )


class LinxiraClient:
    """Client for executing Linxira Bio capabilities.

    Usage:
        client = LinxiraClient()
        result = client.execute("sequence.stats.v1", {"fasta": "tests/fixtures/sequences/tiny.fa"})
        if result.ok:
            print(result.result)
    """

    WORKER_BINARY_NAME = "linxira-bio-worker"

    def __init__(
        self,
        worker_bin: Optional[str] = None,
        base_dir: Optional[str] = None,
    ):
        """Initialize the client.

        Args:
            worker_bin: Path to the linxira-bio-worker binary. If not provided,
                        auto-discovers from PATH, build output, or project root.
            base_dir: Base directory for resolving relative input paths.
                      Defaults to the current working directory.
        """
        self._worker_bin = worker_bin
        self.base_dir = Path(base_dir) if base_dir else Path.cwd()

    @property
    def worker_bin(self) -> str:
        """The resolved path to the linxira-bio-worker binary."""
        if self._worker_bin is None:
            self._worker_bin = self._find_worker()
        return self._worker_bin

    def execute(
        self,
        capability: str,
        inputs: dict,
        parameters: Optional[dict] = None,
        job_id: Optional[str] = None,
    ) -> LinxiraResult:
        """Execute a capability and return the structured result.

        Args:
            capability: The capability ID (e.g. "sequence.stats.v1").
            inputs: Dict mapping input role names to file paths (e.g. {"fasta": "input.fa"}).
            parameters: Optional dict of parameters for the capability.
            job_id: Optional job ID. Auto-generated if not provided.

        Returns:
            LinxiraResult with the parsed result envelope.

        Raises:
            FileNotFoundError: If the worker binary cannot be found.
            subprocess.CalledProcessError: If the worker fails to execute.
            RuntimeError: If the worker returns an error status.
            json.JSONDecodeError: If the worker output is not valid JSON.
        """
        job_id = job_id or str(uuid.uuid4())
        parameters = parameters or {}

        # Resolve relative input paths against base_dir
        resolved_inputs = {}
        for role, path in inputs.items():
            p = Path(path)
            if not p.is_absolute():
                p = self.base_dir / p
            resolved_inputs[role] = str(p.resolve())

        request = {
            "schema_version": "1",
            "job_id": job_id,
            "capability": capability,
            "inputs": resolved_inputs,
            "execution": {"mode": "local-cpu"},
            "parameters": parameters,
        }

        # Write the request to a temporary file
        with tempfile.NamedTemporaryFile(
            mode="w",
            suffix=".json",
            prefix="linxira-bio-job-",
            delete=False,
            encoding="utf-8",
        ) as fh:
            json.dump(request, fh)
            request_path = fh.name

        try:
            process = subprocess.run(
                [self.worker_bin, request_path],
                capture_output=True,
                text=True,
                timeout=600,
                cwd=str(self.base_dir),
            )

            if process.returncode != 0 and not process.stdout.strip():
                raise subprocess.CalledProcessError(
                    process.returncode,
                    [self.worker_bin, request_path],
                    output=process.stdout,
                    stderr=process.stderr,
                )

            if not process.stdout.strip():
                raise RuntimeError(
                    f"Worker produced no output. stderr: {process.stderr.strip()}"
                )

            raw_output = process.stdout
            envelope = json.loads(raw_output)

            result = LinxiraResult(
                job_id=envelope.get("job_id", job_id),
                capability=envelope.get("capability", capability),
                status=envelope.get("status", "error"),
                result=envelope.get("result", {}),
                provenance=envelope.get("provenance", {}),
                warnings=envelope.get("warnings", []),
                raw_json=raw_output,
            )

            if result.status == "error":
                error_msg = "Capability execution returned error status"
                if result.warnings:
                    error_msg += f": {result.warnings}"
                raise RuntimeError(error_msg)

            return result

        finally:
            # Clean up the temporary request file
            try:
                os.unlink(request_path)
            except OSError:
                pass

    def _find_worker(self) -> str:
        """Find the linxira-bio-worker binary.

        Search order:
        1. LINXIRA_BIO_WORKER environment variable
        2. PATH (which on Windows)
        3. Build output directories relative to the project root
        4. Project root release directories
        """
        env_path = os.environ.get("LINXIRA_BIO_WORKER")
        if env_path and Path(env_path).is_file():
            return env_path

        # Check PATH
        import shutil
        which = shutil.which(self.WORKER_BINARY_NAME)
        if which:
            return which

        # On Windows, also check with .exe extension
        if os.name == "nt":
            which_exe = shutil.which(self.WORKER_BINARY_NAME + ".exe")
            if which_exe:
                return which_exe

        # Look relative to the package directory
        package_dir = Path(__file__).resolve().parent
        project_root = package_dir.parent.parent.parent

        # Check common build output locations
        candidates = [
            project_root / "target" / "release" / self.WORKER_BINARY_NAME,
            project_root / "target" / "debug" / self.WORKER_BINARY_NAME,
        ]
        if os.name == "nt":
            candidates.extend([
                project_root / "target" / "release" / (self.WORKER_BINARY_NAME + ".exe"),
                project_root / "target" / "debug" / (self.WORKER_BINARY_NAME + ".exe"),
            ])

        for candidate in candidates:
            if candidate.is_file():
                return str(candidate)

        raise FileNotFoundError(
            f"Could not find {self.WORKER_BINARY_NAME}. "
            f"Set LINXIRA_BIO_WORKER environment variable, "
            f"add it to PATH, or build the project with 'cargo build -p linxira-bio-worker'."
        )