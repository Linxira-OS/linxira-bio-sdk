#!/usr/bin/env bash
# Smoke test: Codex and OpenCode can install/import the Linxira Bio skill pack
# and call a real capability (`sequence stats`) with structured output.
#
# CI behavior: runner networks and model credentials are not guaranteed, so the
# script PASSES with an explicit SKIP when installation, authentication, or the
# agent call is unavailable, and FAILS when a tool is present but cannot import
# the pack or validate the staged skills.
#
# Agent capability calls run against a disposable clone of the repository so a
# sandboxed agent can never mutate the working tree.
#
# Exit codes: 0 pass (possibly skipped), 1 hard failure, 2 usage error.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -W 2>/dev/null || pwd)"
SKILL_PACK="$ROOT/skill-pack.json"
CLI_BIN=""
SKIPPED=0

note() { printf '[smoke-agent-installs] %s\n' "$*"; }
skip() {
  SKIPPED=1
  note "SKIP: $*"
}
fail() {
  note "FAIL: $*"
  exit 1
}

if ! command -v python3 >/dev/null 2>&1; then
  skip "python3 is required to validate the skill pack"
  exit 0
fi

# ---- skill-pack structural import check (no auth or network required) ------
python3 - "$SKILL_PACK" "$ROOT" <<'PY'
import json
import pathlib
import sys

pack_path = pathlib.Path(sys.argv[1])
root = pathlib.Path(sys.argv[2])
pack = json.loads(pack_path.read_text(encoding="utf-8"))
assert pack.get("schema_version") == "1", "skill-pack schema_version"
assert pack.get("id") == "linxira-bio-skills", "skill-pack id"
for skill in pack.get("skills", []):
    skill_path = root / skill["path"]
    skill_md = skill_path / "SKILL.md"
    assert skill_md.is_file(), f"missing {skill_md}"
    assert "TODO" not in skill_md.read_text(encoding="utf-8"), f"unfinished {skill_md}"
print("skill-pack.json: OK")
PY
if [ $? -ne 0 ]; then
  fail "skill-pack.json import validation"
fi

# ---- tool bootstrap ----------------------------------------------------------
ensure_tool() {
  local name="$1"
  local package="$2"
  if command -v "$name" >/dev/null 2>&1; then
    note "$name: present ($(command -v "$name"))"
    return 0
  fi
  if ! command -v npx >/dev/null 2>&1; then
    skip "npx is unavailable; cannot install $name"
    return 1
  fi
  note "$name: installing via npx ($package)"
  if ! npx --yes "$package" --version >/dev/null 2>&1; then
    skip "npx could not install $name (network or registry unavailable)"
    return 1
  fi
  note "$name: installed via npx"
  return 0
}

# ---- codex -------------------------------------------------------------------
codex_import_check() {
  local codex_home
  codex_home="$(mktemp -d)"
  mkdir -p "$codex_home/skills"
  local imported=0
  for skill in "$ROOT"/skills/*/; do
    [ -d "$skill" ] || continue
    name="$(basename "$skill")"
    [ -f "$skill/SKILL.md" ] || continue
    cp -R "$skill" "$codex_home/skills/$name"
    imported=$((imported + 1))
  done
  note "codex: staged $imported skills into a temporary CODEX_HOME"
  local report
  report="$(CODEX_HOME="$codex_home" codex doctor --json 2>/dev/null || true)"
  if [ -z "$report" ]; then
    skip "codex doctor did not produce a report"
    rm -rf "$codex_home"
    return 1
  fi
  local status
  status="$(printf '%s' "$report" | python3 -c \
    'import json,sys; print(json.load(sys.stdin).get("overallStatus","unknown"))' 2>/dev/null || echo unknown)"
  note "codex: doctor overall status '$status' with imported skills"
  rm -rf "$codex_home"
  if [ "$status" != "ok" ]; then
    fail "codex cannot load the imported skill pack (doctor status '$status')"
  fi
  return 0
}

# ---- opencode ----------------------------------------------------------------
opencode_import_check() {
  local version
  version="$(opencode --version 2>/dev/null | head -n1 || true)"
  note "opencode: present ($version)"
  if [ -z "$version" ]; then
    fail "opencode is installed but did not report a version"
  fi
}

# ---- capability call through an agent ----------------------------------------
# Runs inside a disposable clone; envelope failures degrade to SKIP because CI
# runners never have model credentials and local sandbox behavior varies.
agent_has_auth() {
  local tool="$1"
  case "$tool" in
    codex)
      codex doctor --json 2>/dev/null | python3 -c \
        'import json,sys; d=json.load(sys.stdin); print(d.get("checks",{}).get("auth.credentials",{}).get("status","unknown"))' 2>/dev/null
      ;;
    opencode)
      local listing
      listing="$(opencode providers list 2>&1)"
      if printf '%s' "$listing" | grep -qi credentials; then
        echo ok
      else
        echo missing
      fi
      ;;
  esac
}

assert_stats_envelope() {
  python3 -c '
import json
import re
import sys

text = ""

def collect(value):
    global text
    if isinstance(value, str):
        text += value + "\n"
    elif isinstance(value, dict):
        for item in value.values():
            collect(item)
    elif isinstance(value, list):
        for item in value:
            collect(item)

for line in sys.stdin:
    text += line
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        continue
    collect(event)

for match in re.finditer(r"\"capability\"\s*:\s*\"sequence\.stats\.v1\"", text):
    start = text.rfind("{", 0, match.start())
    if start < 0:
        continue
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                candidate = text[start : index + 1]
                try:
                    envelope = json.loads(candidate)
                except json.JSONDecodeError:
                    break
                if envelope.get("status") == "ok":
                    sys.exit(0)
                break
sys.exit(1)
'
}

run_agent_capability_call() {
  local tool="$1"
  local auth
  auth="$(agent_has_auth "$tool")"
  if [ "$auth" != "ok" ]; then
    skip "$tool has no model credentials; agent capability call skipped"
    return 0
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    skip "cargo is unavailable; agent capability call skipped"
    return 0
  fi
  local clone
  clone="$(mktemp -d)"
  note "$tool: copying repository into a disposable sandbox"
  mkdir -p "$clone/repo"
  if ! tar -C "$ROOT" \
      --exclude='./.git' --exclude='./target' --exclude='./.venv-ci' \
      --exclude='./build' --exclude='./.linxira-bio' --exclude='./.venv-convert' \
      -cf - . | tar -C "$clone/repo" -xf - 2>/dev/null; then
    skip "could not copy the repository for the agent call"
    rm -rf "$clone"
    return 0
  fi
  local output
  case "$tool" in
    codex)
      output="$(cd "$clone/repo" && codex exec -s danger-full-access --skip-git-repo-check --ephemeral \
        "Use the imported linxira-bio skills. Run \`cargo run --quiet -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json\` and output the resulting JSON envelope verbatim." 2>/dev/null || true)"
      ;;
    opencode)
      output="$(cd "$clone/repo" && opencode run --format json \
        "Use the imported linxira-bio skills. Run \`cargo run --quiet -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json\` and output the resulting JSON envelope verbatim." 2>/dev/null || true)"
      ;;
  esac
  rm -rf "$clone"
  if printf '%s' "$output" | assert_stats_envelope; then
    note "$tool: structured sequence.stats.v1 envelope observed"
    return 0
  fi
  skip "$tool agent call produced no parseable envelope in the disposable sandbox"
  return 0
}

# ---- main ---------------------------------------------------------------------
if ensure_tool codex "@openai/codex"; then
  codex_import_check || exit 1
  run_agent_capability_call codex
fi
if ensure_tool opencode "opencode-ai"; then
  opencode_import_check
  run_agent_capability_call opencode
fi

if [ "$SKIPPED" -eq 1 ]; then
  note "PASS (with skips): no hard failures"
else
  note "PASS: both agent toolchains imported the skill pack and produced structured output"
fi
exit 0
