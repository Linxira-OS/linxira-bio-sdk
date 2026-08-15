"""Linxira Bio SDK - Programmatic access to bioinformatics capabilities."""

from .catalog import get_capability, get_catalog, list_capabilities
from .client import LinxiraClient, LinxiraResult
from .dynamic import AutoClient, DynamicAPI
from .environment import (
    ApplyResult,
    AuditResult,
    EnvironmentManager,
    InstallPlan,
    PlanAction,
    PlatformInfo,
    ToolStatus,
)
from .workflow import (
    WorkflowManager,
    QuickAnalysis,
)

__all__ = [
    "AutoClient",
    "LinxiraClient",
    "LinxiraResult",
    "DynamicAPI",
    "EnvironmentManager",
    "AuditResult",
    "InstallPlan",
    "ApplyResult",
    "PlanAction",
    "PlatformInfo",
    "ToolStatus",
    "WorkflowManager",
    "QuickAnalysis",
    "get_catalog",
    "get_capability",
    "list_capabilities",
]
__version__ = "0.1.0"