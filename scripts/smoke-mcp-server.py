#!/usr/bin/env python3
"""Smoke test for the Linxira Bio MCP server (JSON-RPC over stdio).

Launches the `linxira-bio-mcp` binary, sends `initialize`, `tools/list`, and
`resources/read` over stdin, and asserts that the tool list exposes catalog
capabilities and that reading a capability document returns its text. The
server must exit 0 when stdin closes.

Usage:
    smoke-mcp-server.py [--bin PATH] [--worker PATH] [--doc PATH]
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def find_binary(name: str, env_name: str) -> Path:
    configured = os.environ.get(env_name)
    if configured:
        return Path(configured)
    repo_root = Path(__file__).resolve().parents[1]
    candidates = []
    for profile in ("release", "debug"):
        for suffix in ("", ".exe"):
            candidate = repo_root / "target" / profile / (name + suffix)
            if candidate.is_file():
                candidates.append(candidate)
    if not candidates:
        raise SystemExit(f"{env_name} not set and no target/{name} binary found")
    # Prefer the most recently built binary: a stale release snapshot can
    # predate the runtime-catalog loading under test.
    return max(candidates, key=lambda path: path.stat().st_mtime)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", dest="binary")
    parser.add_argument("--worker", dest="worker")
    parser.add_argument("--doc", dest="doc")
    args = parser.parse_args()

    binary = Path(args.binary) if args.binary else find_binary("linxira-bio-mcp", "LINXIRA_BIO_MCP_BIN")
    worker = Path(args.worker) if args.worker else find_binary("linxira-bio-worker", "LINXIRA_BIO_WORKER_BIN")
    repo_root = Path(__file__).resolve().parents[1]
    doc = Path(args.doc) if args.doc else repo_root / "docs/capabilities/sequence.stats.v1/en-US.md"
    if not doc.is_file():
        raise SystemExit(f"capability document not found: {doc}")
    # file:/// + posix: Windows "C:/..." (3-slash URI, strips to absolute);
    # POSIX "/__w/..." yields "file:////__w/..." (4-slash), server strips
    # "file:///" leaving the leading "/" so the absolute path is preserved.
    doc_uri = "file:///" + doc.resolve().as_posix()

    # Runtime catalog override: point LINXIRA_BIO_WORKFLOW_ROOT at a directory
    # with a single marker capability and assert tools/list reflects it.
    catalog_root = Path(tempfile.mkdtemp(prefix="linxira-bio-mcp-catalog-"))
    capabilities_dir = catalog_root / "capabilities"
    capabilities_dir.mkdir()
    (capabilities_dir / "catalog.json").write_text(
        json.dumps({
            "schema_version": "1",
            "product": "linxira-bio-sdk",
            "capabilities": [{
                "id": "test.mcp-runtime-catalog.v1",
                "status": "available",
                "category": "system",
                "default_execution": "local-cpu",
                "command": "linxira-bio capabilities --json",
            }],
        }),
        encoding="utf-8",
    )

    env = dict(os.environ)
    env["CARGO_BIN_EXE_linxira-bio-worker"] = str(worker)
    env["LINXIRA_BIO_WORKFLOW_ROOT"] = str(catalog_root)
    process = subprocess.Popen(
        [str(binary)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    requests = [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
        {"jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": {"uri": doc_uri}},
    ]
    try:
        for request in requests:
            process.stdin.write(json.dumps(request) + "\n")
        process.stdin.flush()
        process.stdin.close()

        responses = {}
        for line in process.stdout:
            line = line.strip()
            if not line:
                continue
            message = json.loads(line)
            responses[message.get("id")] = message

        stderr = process.stderr.read()
        exit_code = process.wait()
    except Exception as error:  # noqa: BLE001 - report and kill on any driver failure
        process.kill()
        raise SystemExit(f"mcp smoke driver failed: {error}")
    finally:
        shutil.rmtree(catalog_root, ignore_errors=True)

    if exit_code != 0:
        print(f"FAIL: mcp server exited {exit_code}; stderr: {stderr[-2000:]}")
        return 1

    tools_response = responses.get(2)
    tools = ((tools_response or {}).get("result") or {}).get("tools") or []
    capability_tool_names = {tool.get("name") for tool in tools}
    if "test_mcp-runtime-catalog_v1" not in capability_tool_names:
        print(
            "FAIL: tools/list did not reflect the runtime catalog; "
            f"names: {sorted(capability_tool_names)[:10]}"
        )
        return 1
    if len(tools) != 1:
        print(f"FAIL: tools/list should expose exactly the runtime catalog marker, got {len(tools)}")
        return 1

    read_response = responses.get(3)
    contents = ((read_response or {}).get("result") or {}).get("contents") or []
    text = contents[0].get("text", "") if contents else ""
    if "FASTA Sequence Statistics" not in text or len(text) < 100:
        print("FAIL: resources/read did not return the sequence.stats.v1 document")
        return 1

    print(
        "PASS: mcp server loaded the runtime catalog (1 tool), "
        f"returned {len(text)} bytes of capability documentation, exited 0"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
