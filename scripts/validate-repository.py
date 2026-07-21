#!/usr/bin/env python3
"""Validate the dependency-free repository contracts used in CI."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_json(relative_path: str) -> object:
    path = ROOT / relative_path
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def parse_skill_frontmatter(path: Path) -> dict[str, str]:
    text = path.read_text(encoding="utf-8")
    if "TODO" in text:
        raise ValueError(f"unfinished TODO in {path.relative_to(ROOT)}")

    parts = text.split("---", 2)
    if len(parts) != 3 or parts[0].strip():
        raise ValueError(f"invalid frontmatter boundary in {path.relative_to(ROOT)}")

    fields: dict[str, str] = {}
    for line in parts[1].splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        fields[key.strip()] = value.strip()
    return fields


def validate() -> None:
    catalog = load_json("capabilities/catalog.json")
    tool_catalog = load_json("tools/catalog.json")
    pack = load_json("skill-pack.json")
    profile = load_json("profiles/local-core.json")
    load_json("schemas/capability.schema.json")
    load_json("schemas/job-request.schema.json")
    load_json("schemas/analysis-result.schema.json")
    load_json("schemas/tool-catalog.schema.json")

    license_text = (ROOT / "LICENSE").read_text(encoding="utf-8")
    if "GNU AFFERO GENERAL PUBLIC LICENSE" not in license_text:
        raise ValueError("LICENSE is not the full GNU Affero GPL text")
    if 'license = "AGPL-3.0-or-later"' not in (ROOT / "Cargo.toml").read_text(
        encoding="utf-8"
    ):
        raise ValueError("Cargo workspace license is not AGPL-3.0-or-later")
    if not (ROOT / "deny.toml").is_file():
        raise ValueError("deny.toml is required")

    if not isinstance(catalog, dict) or catalog.get("schema_version") != "1":
        raise ValueError("invalid capability catalog header")
    if not isinstance(tool_catalog, dict) or tool_catalog.get("schema_version") != "1":
        raise ValueError("invalid tool catalog header")
    if not isinstance(pack, dict) or pack.get("schema_version") != "1":
        raise ValueError("invalid skill-pack header")
    if pack.get("license") != "AGPL-3.0-or-later":
        raise ValueError("skill-pack license is not AGPL-3.0-or-later")
    notices = pack.get("third_party_notices")
    if not isinstance(notices, str) or not (ROOT / notices).is_file():
        raise ValueError("skill-pack third-party notice file is missing")
    if not isinstance(profile, dict) or profile.get("schema_version") != "1":
        raise ValueError("invalid profile header")

    capabilities = catalog.get("capabilities")
    if not isinstance(capabilities, list):
        raise ValueError("capabilities must be a list")

    capability_ids: set[str] = set()
    for capability in capabilities:
        if not isinstance(capability, dict):
            raise ValueError("capability entries must be objects")
        capability_id = capability.get("id")
        if not isinstance(capability_id, str) or not capability_id:
            raise ValueError("capability id is required")
        if capability_id in capability_ids:
            raise ValueError(f"duplicate capability id: {capability_id}")
        capability_ids.add(capability_id)
        if capability.get("status") == "available" and not capability.get("command"):
            raise ValueError(f"available capability lacks a command: {capability_id}")

    tools = tool_catalog.get("tools")
    tool_profiles = tool_catalog.get("profiles")
    if not isinstance(tools, list) or not isinstance(tool_profiles, list):
        raise ValueError("tool catalog requires tools and profiles lists")

    tool_ids: set[str] = set()
    for tool in tools:
        if not isinstance(tool, dict):
            raise ValueError("tool entries must be objects")
        tool_id = tool.get("id")
        probes = tool.get("probes")
        install = tool.get("install")
        if not isinstance(tool_id, str) or not tool_id:
            raise ValueError("tool id is required")
        if tool_id in tool_ids:
            raise ValueError(f"duplicate tool id: {tool_id}")
        if not isinstance(probes, list) or not probes:
            raise ValueError(f"tool requires at least one probe: {tool_id}")
        if not isinstance(install, dict) or not set(install) <= {
            "windows",
            "debian",
            "arch",
        }:
            raise ValueError(f"invalid install platform for tool: {tool_id}")
        tool_ids.add(tool_id)

    environment_profile_ids: set[str] = set()
    for environment_profile in tool_profiles:
        if not isinstance(environment_profile, dict):
            raise ValueError("environment profile entries must be objects")
        profile_id = environment_profile.get("id")
        profile_tools = environment_profile.get("tools")
        if not isinstance(profile_id, str) or not profile_id:
            raise ValueError("environment profile id is required")
        if profile_id in environment_profile_ids:
            raise ValueError(f"duplicate environment profile: {profile_id}")
        if not isinstance(profile_tools, list) or not set(profile_tools) <= tool_ids:
            raise ValueError(f"environment profile references unknown tool: {profile_id}")
        environment_profile_ids.add(profile_id)

    pack_skills = pack.get("skills")
    if not isinstance(pack_skills, list):
        raise ValueError("skill-pack skills must be a list")

    skill_ids: set[str] = set()
    for descriptor in pack_skills:
        if not isinstance(descriptor, dict):
            raise ValueError("skill descriptor must be an object")
        skill_id = descriptor.get("id")
        relative_path = descriptor.get("path")
        if not isinstance(skill_id, str) or not isinstance(relative_path, str):
            raise ValueError("skill id and path are required")
        if skill_id in skill_ids:
            raise ValueError(f"duplicate skill id: {skill_id}")
        skill_ids.add(skill_id)

        skill_root = ROOT / relative_path
        skill_file = skill_root / "SKILL.md"
        interface_file = skill_root / "agents/openai.yaml"
        if not skill_file.is_file() or not interface_file.is_file():
            raise ValueError(f"incomplete skill folder: {relative_path}")

        fields = parse_skill_frontmatter(skill_file)
        if fields.get("name") != skill_id:
            raise ValueError(f"skill name mismatch: {skill_id}")
        if not fields.get("description"):
            raise ValueError(f"skill description missing: {skill_id}")

        interface = interface_file.read_text(encoding="utf-8")
        if f"${skill_id}" not in interface:
            raise ValueError(f"default prompt does not mention ${skill_id}")

        for capability_id in descriptor.get("capabilities", []):
            if capability_id not in capability_ids:
                raise ValueError(
                    f"skill {skill_id} references unknown capability {capability_id}"
                )

    profile_skills = profile.get("skills")
    if not isinstance(profile_skills, list) or not set(profile_skills) <= skill_ids:
        raise ValueError("profile references an unknown skill")

    required = profile.get("required_capabilities")
    if not isinstance(required, list) or not set(required) <= capability_ids:
        raise ValueError("profile references an unknown capability")

    print(
        f"validated {len(capability_ids)} capabilities, "
        f"{len(skill_ids)} skills, {len(tool_ids)} tools, "
        f"and profile {profile.get('id')}"
    )


if __name__ == "__main__":
    validate()
