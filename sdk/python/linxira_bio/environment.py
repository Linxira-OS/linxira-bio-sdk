"""Intelligent environment management with cross-platform auto-completion.

Provides platform detection, environment auditing, installation planning,
and one-shot environment setup across Windows, Debian, and Arch systems.
"""

import json
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any, Optional

from .catalog import get_capability


class PlatformInfo:
    """Detected platform information."""

    def __init__(self, os_name: str, family: str, arch: str, supported: bool):
        self.os_name = os_name
        self.family = family
        self.arch = arch
        self.supported = supported

    def __repr__(self) -> str:
        return (
            f"PlatformInfo(os={self.os_name!r}, family={self.family!r}, "
            f"arch={self.arch!r}, supported={self.supported})"
        )

    @property
    def is_windows(self) -> bool:
        return self.family == "windows"

    @property
    def is_debian(self) -> bool:
        return self.family == "debian"

    @property
    def is_arch(self) -> bool:
        return self.family == "arch"

    @property
    def is_macos(self) -> bool:
        return self.family == "darwin"


class ToolStatus:
    """Status of a single tool in the environment."""

    def __init__(
        self,
        tool_id: str,
        display_name: str,
        category: str,
        available: bool,
        command: Optional[str] = None,
        version: Optional[str] = None,
        discovered_outside_path: bool = False,
    ):
        self.tool_id = tool_id
        self.display_name = display_name
        self.category = category
        self.available = available
        self.command = command
        self.version = version
        self.discovered_outside_path = discovered_outside_path

    def __repr__(self) -> str:
        status = "available" if self.available else "missing"
        return f"ToolStatus({self.tool_id!r}, {status})"


class AuditResult:
    """Result of an environment audit."""

    def __init__(self, raw: dict):
        self._raw = raw
        self.platform = PlatformInfo(**raw.get("platform", {}))
        self.tools = [
            ToolStatus(
                tool_id=t.get("id", ""),
                display_name=t.get("display_name", ""),
                category=t.get("category", ""),
                available=t.get("available", False),
                command=t.get("command"),
                version=t.get("version"),
                discovered_outside_path=t.get("discovered_outside_path", False),
            )
            for t in raw.get("tools", [])
        ]
        summary = raw.get("summary", {})
        self.available_count = summary.get("available", 0)
        self.missing_count = summary.get("missing", 0)
        self.warnings = raw.get("warnings", [])

    @property
    def missing_tools(self) -> list:
        """List of tools that are not installed."""
        return [t for t in self.tools if not t.available]

    @property
    def available_tools(self) -> list:
        """List of tools that are installed."""
        return [t for t in self.tools if t.available]

    @property
    def is_ready(self) -> bool:
        """True if all tools are available."""
        return self.missing_count == 0

    def to_dict(self) -> dict:
        return self._raw


class PlanAction:
    """A single action in an installation plan."""

    def __init__(self, raw: dict):
        self._raw = raw
        self.tool_id = raw.get("tool_id", "")
        self.display_name = raw.get("display_name", "")
        self.state = raw.get("state", "missing")
        self.strategy = raw.get("strategy", "unknown")
        self.package = raw.get("package")
        self.reason = raw.get("reason")

    @property
    def needs_install(self) -> bool:
        return self.state == "install"

    @property
    def is_available(self) -> bool:
        return self.state == "available"

    def __repr__(self) -> str:
        return f"PlanAction({self.tool_id!r}, {self.state})"


class InstallPlan:
    """An installation plan for the environment."""

    def __init__(self, raw: dict):
        self._raw = raw
        self.profile = raw.get("profile", "")
        self.platform = raw.get("platform", "")
        self.actions = [PlanAction(a) for a in raw.get("actions", [])]
        self.blockers = raw.get("transaction", {}).get("blockers", [])

    @property
    def install_actions(self) -> list:
        """Actions that require installation."""
        return [a for a in self.actions if a.needs_install]

    @property
    def available_actions(self) -> list:
        """Actions that are already available."""
        return [a for a in self.actions if a.is_available]

    @property
    def has_blockers(self) -> bool:
        return len(self.blockers) > 0

    @property
    def ready(self) -> bool:
        """True if no installation is needed and no blockers exist."""
        return len(self.install_actions) == 0 and not self.has_blockers

    def to_dict(self) -> dict:
        return self._raw


class ApplyResult:
    """Result of applying an installation plan."""

    def __init__(self, raw: dict):
        self._raw = raw
        self.profile = raw.get("profile", "")
        self.platform = raw.get("platform", "")
        summary = raw.get("summary", {})
        self.installed_count = summary.get("installed", 0)
        self.failed_count = summary.get("failed", 0)
        self.skipped_count = summary.get("skipped", 0)
        self.total_count = summary.get("total", 0)
        self.installed = raw.get("installed", [])
        self.failed = raw.get("failed", [])

    @property
    def ok(self) -> bool:
        return self.failed_count == 0

    def to_dict(self) -> dict:
        return self._raw


# Known profile descriptions for intelligent suggestion
_PROFILE_DESCRIPTIONS = {
    "local-core": "Built-in Rust capabilities (no external tools needed)",
    "scripting": "Python, R, and Java runtimes",
    "managed-runtimes": "User-scoped runtime managers (uv, pixi, rig, miniforge)",
    "containers": "Container and Unix execution backends (Docker, Podman, WSL)",
    "sequence-search": "BLAST+, DIAMOND, HMMER for sequence similarity search",
    "comparative-genomics": "MCScanX and KaKs Calculator for comparative genomics",
    "multiple-sequence-alignment": "MUSCLE and trimAl for MSA",
    "phylogenetics": "IQ-TREE for phylogenetic inference",
    "motif-analysis": "MEME for motif discovery",
    "protein-structure": "DSSP for protein structure annotation",
    "genomics-cli": "samtools, bcftools, bedtools, minimap2, snpEff",
    "full-local": "All registered local runtimes and bioinformatics tools",
}

# Capability-to-profile mapping for intelligent suggestion
_CAPABILITY_PROFILE_MAP = {
    "similarity.blast": "sequence-search",
    "similarity.diamond": "sequence-search",
    "similarity.hmmer": "sequence-search",
    "comparative.mcscanx": "comparative-genomics",
    "comparative.kaks": "comparative-genomics",
    "msa.muscle": "multiple-sequence-alignment",
    "msa.trimal": "multiple-sequence-alignment",
    "phylogeny.iqtree": "phylogenetics",
    "motif.meme": "motif-analysis",
    "protein.secondary-structure": "protein-structure",
    "alignment.short-read": "genomics-cli",
    "alignment.long-read": "genomics-cli",
    "alignment.bam-cram-qc": "genomics-cli",
    "alignment.coverage": "genomics-cli",
    "variant.annotate": "genomics-cli",
    "interval.intersect": "genomics-cli",
    "interval.merge": "genomics-cli",
    "interval.subtract": "genomics-cli",
    "interval.closest": "genomics-cli",
    "expression.differential": "scripting",
    "expression.wgcna": "scripting",
}


class EnvironmentManager:
    """Manages the bioinformatics environment across platforms.

    Handles platform detection, auditing, installation planning, and
    one-shot environment setup with intelligent profile suggestion.

    Usage:
        env_mgr = EnvironmentManager(client)
        audit = env_mgr.audit()
        if not audit.is_ready:
            plan = env_mgr.plan("genomics-cli")
            result = env_mgr.apply("genomics-cli")
    """

    def __init__(self, client):
        """Initialize with a LinxiraClient instance.

        Args:
            client: A LinxiraClient for executing environment capabilities.
        """
        self._client = client

    @staticmethod
    def detect_platform() -> PlatformInfo:
        """Detect the current platform.

        Returns:
            PlatformInfo with OS family, architecture, and support status.
        """
        system = platform.system().lower()
        machine = platform.machine().lower()

        if system == "windows":
            family = "windows"
        elif system == "linux":
            # Detect distribution family
            try:
                result = subprocess.run(
                    ["cat", "/etc/os-release"],
                    capture_output=True, text=True, timeout=5
                )
                os_release = result.stdout.lower()
                if "debian" in os_release or "ubuntu" in os_release:
                    family = "debian"
                elif "arch" in os_release:
                    family = "arch"
                else:
                    family = "linux-other"
            except Exception:
                family = "linux-other"
        elif system == "darwin":
            family = "darwin"
        else:
            family = system

        supported = family in ("windows", "debian", "arch", "darwin")

        return PlatformInfo(
            os_name=system,
            family=family,
            arch=machine,
            supported=supported,
        )

    def audit(self) -> AuditResult:
        """Run an environment audit to check installed tools.

        Returns:
            AuditResult with tool availability and platform information.
        """
        result = self._client.execute("environment.audit.v1", {})
        return AuditResult(result.result)

    def plan(self, profile: str = "local-core") -> InstallPlan:
        """Generate an installation plan for a profile.

        Args:
            profile: Profile ID (e.g. "genomics-cli", "full-local",
                     "sequence-search"). Defaults to "local-core".

        Returns:
            InstallPlan with actions for each tool.
        """
        result = self._client.execute(
            "environment.plan.v1",
            {},
            parameters={"profile": profile},
        )
        return InstallPlan(result.result)

    def apply(
        self,
        profile: str,
        mode: str = "interactive",
        project_root: Optional[str] = None,
    ) -> ApplyResult:
        """Execute an installation plan.

        Requires explicit user approval before installing system packages.

        Args:
            profile: Profile ID to install.
            mode: Installation mode ("interactive", "auto", "dry-run").
            project_root: Optional project root for scoped installation.

        Returns:
            ApplyResult with installation summary.
        """
        params = {"profile": profile, "mode": mode}
        if project_root:
            params["project_root"] = project_root

        result = self._client.execute(
            "environment.apply.v1",
            {},
            parameters=params,
        )
        return ApplyResult(result.result)

    def ensure(
        self,
        profile: str,
        mode: str = "interactive",
        auto_apply: bool = False,
    ) -> ApplyResult:
        """One-shot environment setup: audit → plan → apply if needed.

        Args:
            profile: Profile ID to ensure.
            mode: Installation mode.
            auto_apply: If True, automatically apply the plan if tools are
                       missing. If False, return the plan without applying.

        Returns:
            ApplyResult if applied, or raises a helpful message if tools
            are missing and auto_apply is False.
        """
        audit = self.audit()
        if audit.is_ready:
            return ApplyResult({
                "profile": profile,
                "platform": audit.platform.family,
                "summary": {
                    "installed": audit.available_count,
                    "failed": 0,
                    "skipped": 0,
                    "total": audit.available_count + audit.missing_count,
                },
                "installed": [],
                "failed": [],
            })

        plan = self.plan(profile)
        if plan.ready:
            return ApplyResult({
                "profile": profile,
                "platform": plan.platform,
                "summary": {
                    "installed": len(plan.available_actions),
                    "failed": 0,
                    "skipped": 0,
                    "total": len(plan.actions),
                },
                "installed": [],
                "failed": [],
            })

        if not auto_apply:
            missing = [a.display_name for a in plan.install_actions]
            msg = (
                f"Environment profile '{profile}' is missing {len(missing)} "
                f"tool(s): {', '.join(missing)}.\n"
                f"Run ensure(profile, auto_apply=True) to install them, or "
                f"call apply('{profile}') directly."
            )
            raise EnvironmentError(msg)

        return self.apply(profile, mode=mode)

    def suggest_profile(self, capabilities: list) -> list:
        """Suggest the minimal profiles needed for a set of capabilities.

        Args:
            capabilities: List of capability IDs to check.

        Returns:
            List of unique profile IDs needed.
        """
        profiles = set()
        profiles.add("local-core")  # Always needed

        for cap_id in capabilities:
            # Check prefix matches (e.g. "similarity.blast.local" → "similarity.blast")
            for prefix, profile in _CAPABILITY_PROFILE_MAP.items():
                if cap_id.startswith(prefix):
                    profiles.add(profile)
                    break

        return sorted(profiles)

    @staticmethod
    def list_profiles() -> dict:
        """List all available environment profiles with descriptions.

        Returns:
            Dict mapping profile IDs to descriptions.
        """
        return dict(_PROFILE_DESCRIPTIONS)

    def check_capability_readiness(self, capability_id: str) -> dict:
        """Check if the environment is ready for a specific capability.

        Args:
            capability_id: Capability ID to check.

        Returns:
            Dict with 'ready' (bool), 'missing_profiles' (list), and
            'suggested_command' (str) fields.
        """
        profiles = self.suggest_profile([capability_id])
        missing = []

        for profile in profiles:
            if profile == "local-core":
                continue
            plan = self.plan(profile)
            if not plan.ready:
                missing.append(profile)

        if missing:
            return {
                "ready": False,
                "capability": capability_id,
                "missing_profiles": missing,
                "suggested_command": (
                    f"env_mgr.ensure('{' '.join(missing)}', auto_apply=True)"
                ),
            }

        return {
            "ready": True,
            "capability": capability_id,
            "missing_profiles": [],
        }