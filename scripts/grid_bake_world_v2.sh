#!/usr/bin/env bash
set -euo pipefail

# OxideSwarm Grid Bake Orchestrator for World v2 (`JOB-W0 .. JOB-W6`, Design §6.1-§6.2).
# Enforces `AGENT_RULES.md §4` (`QD-OXS`):
#   - Always passes `--worker <GRID_WORKER>` (default `desktop-3epv830`, never omitted)
#   - Refuses to submit if `WORK_PLAN.md` says `GRID: PAUSED`
#   - Refuses to submit unless worker shows `state:connected`, `feat:workspaces`, `tool:cargo`
#   - Refuses to submit while `anima-case` activity is `active`
#   - Uses registered workspace (`anima-t1` or `anima-t2`) with bounded `--timeout-secs`
#   - Runs `JOB-W0` (`probe-gpu`) before attempting GPU `JOB-W4` (`--backend wgpu`) or `JOB-W6`

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=1
  shift
fi

SUBCMD="${1:-all}"
shift || true

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OXS_RUN="${OXIDESWARM_RUN:-$HOME/OxideSwarm-trial/run.sh}"
HUB_DIR="${ANIMA_HUB_DIR:-$HOME/Library/CloudStorage/GoogleDrive-duonganhdn2000@gmail.com/Drive của tôi/Anima_Agent_Hub}"
WORKER="${GRID_WORKER:-desktop-3epv830}"
WORKSPACE="${GRID_WORKSPACE:-anima-t1}"
PRESET_REL="${WORLD_V2_PRESET:-artifacts/world-v2/preset_v2.toml}"
RELEASE_TAG="${WORLD_V2_RELEASE_TAG:-world-v2-c01-provisional}"
OUT_DIR="${GRID_OUT_REL:-target/grid_world_v2}"
STRIPS="${GRID_FIELD_STRIPS:-4}"

usage() {
  cat <<'USAGE'
Usage: scripts/grid_bake_world_v2.sh [--dry-run] <probe-gpu|generate|verify|fields|lighting|pack|assets|all>

Environment overrides:
  GRID_WORKER          Target worker name (default: desktop-3epv830; always required)
  GRID_WORKSPACE       Registered workspace (anima-t1 or anima-t2; default: anima-t1)
  GRID_FIELD_STRIPS    Strip count K for JOB-W3 sharding (default: 4)
  WORLD_V2_PRESET      Preset TOML path relative to repo root (default: artifacts/world-v2/preset_v2.toml)
  WORLD_V2_RELEASE_TAG Tag recorded into index.json (default: world-v2-c01-provisional)
USAGE
}

if [[ "$SUBCMD" == "-h" || "$SUBCMD" == "--help" ]]; then
  usage
  exit 0
fi

case "$WORKSPACE" in
  anima-t1|anima-t2|anima-trunk|anima-verify|anima-worldbake) ;;
  *)
    echo "ERROR: unregistered workspace '$WORKSPACE' (use anima-t1 or anima-t2)" >&2
    exit 2
    ;;
esac

if [[ -z "$WORKER" ]]; then
  echo "ERROR: GRID_WORKER must not be empty (QD-OXS)" >&2
  exit 2
fi

preflight_checks() {
  if [[ -f "$HUB_DIR/WORK_PLAN.md" ]]; then
    if grep -qE '^GRID:\s*PAUSED' "$HUB_DIR/WORK_PLAN.md"; then
      echo "GRID PAUSED in $HUB_DIR/WORK_PLAN.md; refusing submission" >&2
      exit 3
    fi
  fi

  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] preflight: WORKER=$WORKER WORKSPACE=$WORKSPACE PRESET=$PRESET_REL"
    return 0
  fi

  if [[ ! -x "$OXS_RUN" ]]; then
    echo "GRID NOT READY: missing $OXS_RUN" >&2
    exit 3
  fi

  local workers_json
  workers_json="$("$OXS_RUN" workers --json)"
  python3 - "$workers_json" "$WORKER" <<'PY'
import json, sys
data = json.loads(sys.argv[1])
target = sys.argv[2]
workers = data if isinstance(data, list) else data.get("workers", [])
match = next((w for w in workers if w.get("name") == target or w.get("id") == target), None)
if not match:
    sys.exit(f"GRID NOT READY: worker '{target}' not found")
state = match.get("state", "")
feats = match.get("features", [])
tools = match.get("tools", [])
if state != "connected" or "workspaces" not in feats or "cargo" not in tools:
    sys.exit(f"GRID NOT READY: worker '{target}' state={state} features={feats} tools={tools}")
print(f"[grid] worker {target} ready (state={state}, features={feats}, tools={tools})")
PY

  if [[ -f "$HUB_DIR/anima_bridge.py" ]]; then
    if python3 "$HUB_DIR/anima_bridge.py" --agent-id anima-mac activity --show 2>/dev/null | grep -qiE 'agent-anima-case.*ACTIVE'; then
      echo "ERROR: anima-case is currently ACTIVE on $WORKER; wait until idle/blocked/done" >&2
      exit 3
    fi
  fi
}

submit_cmd() {
  local label="$1"
  local cmd_str="$2"
  echo "[grid:$label] worker=$WORKER workspace=$WORKSPACE cmd: $cmd_str"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $OXS_RUN submit --worker $WORKER --workspace $WORKSPACE --sync . --cwd src-tauri --require-tool cargo --timeout-secs 3600 --wait --command cmd -- /c \"$cmd_str\""
    return 0
  fi
  (
    cd "$REPO_ROOT"
    "$OXS_RUN" submit \
      --worker "$WORKER" \
      --workspace "$WORKSPACE" \
      --sync . \
      --cwd src-tauri \
      --require-tool cargo \
      --timeout-secs 3600 \
      --env CARGO_BUILD_JOBS=4 \
      --wait \
      --command cmd -- /c "$cmd_str"
  )
}

run_w0_probe() {
  submit_cmd "JOB-W0" \
    "cargo run --release -p anima-worldgen --features gpu-bake --bin anima-bake -- probe-gpu 2>&1"
}

run_w1_generate() {
  submit_cmd "JOB-W1" \
    "cargo run --release -p anima-worldgen --features generate --bin anima-worldgen -- generate --preset ../$PRESET_REL --out $OUT_DIR/raw 2>&1"
}

run_w2_verify() {
  submit_cmd "JOB-W2" \
    "cargo run --release -p anima-worldgen --features generate --bin anima-worldgen -- verify --raw $OUT_DIR/raw --report $OUT_DIR/gates.json --out-sim $OUT_DIR/sim_256.anmw3 --out-index $OUT_DIR/index_raw.json --report-only 2>&1"
}

run_w3_fields() {
  local s
  for ((s = 0; s < STRIPS; s++)); do
    submit_cmd "JOB-W3-strip-$s" \
      "cargo run --release -p anima-worldgen --features bake --bin anima-bake -- fields --raw $OUT_DIR/raw --tile-strip ${s}/${STRIPS} --out $OUT_DIR/baked 2>&1"
  done
  submit_cmd "JOB-W3-merge" \
    "cargo run --release -p anima-worldgen --features bake --bin anima-bake -- fields --out $OUT_DIR/baked --merge-strips ${STRIPS} 2>&1"
}

run_w4_lighting() {
  local probe_done="${1:-0}"
  if [[ "${FORCE_CPU_LIGHTING:-0}" == "0" ]]; then
    if [[ "$probe_done" -eq 1 ]] || run_w0_probe; then
      submit_cmd "JOB-W4-wgpu" \
        "cargo run --release -p anima-worldgen --features gpu-bake --bin anima-bake -- lighting --backend wgpu --raw $OUT_DIR/raw --samples 64 --out $OUT_DIR/baked 2>&1"
      return 0
    fi
    echo "[grid] JOB-W0 probe-gpu unavailable; falling back cleanly to JOB-W4 --backend cpu" >&2
  fi
  submit_cmd "JOB-W4-cpu" \
    "cargo run --release -p anima-worldgen --features bake --bin anima-bake -- lighting --backend cpu --raw $OUT_DIR/raw --out $OUT_DIR/baked 2>&1"
}

run_w5_pack() {
  # Packages unified $OUT_DIR/baked (fields + lighting) into release tar.gz (no index.json inside archive).
  submit_cmd "JOB-W5" \
    "cargo run --release -p anima-worldgen --features bake --bin anima-bake -- pack --in $OUT_DIR/baked --raw $OUT_DIR/raw --sim $OUT_DIR/sim_256.anmw3 --release-tag $RELEASE_TAG --out $OUT_DIR/world_v2 --verify 2>&1"
}

run_w6_assets() {
  # Note (`QD-ASSETS`): artifacts/assets-v2/sources.json is ONLY staged after explicit owner approval (GATE a).
  if [[ "$DRY_RUN" -eq 0 && ! -f "$REPO_ROOT/artifacts/assets-v2/sources.json" ]]; then
    echo "BLOCKED GATE (a): artifacts/assets-v2/sources.json not staged; owner CC0 asset approval required first" >&2
    exit 4
  fi
  run_w0_probe || true
  submit_cmd "JOB-W6" \
    "cargo run --release -p anima-worldgen --features bake --bin anima-bake -- assets --manifest ../artifacts/assets-v2/sources.json --out $OUT_DIR/assets_baked 2>&1"
}

preflight_checks

case "$SUBCMD" in
  probe-gpu) run_w0_probe ;;
  generate) run_w1_generate ;;
  verify) run_w2_verify ;;
  fields) run_w3_fields ;;
  lighting) run_w4_lighting 0 ;;
  pack) run_w5_pack ;;
  assets) run_w6_assets ;;
  all)
    probe_ok=0
    if run_w0_probe; then
      probe_ok=1
    fi
    run_w1_generate
    run_w2_verify
    run_w3_fields
    if [[ "$probe_ok" -eq 1 ]]; then
      run_w4_lighting 1
    else
      FORCE_CPU_LIGHTING=1 run_w4_lighting 0
    fi
    run_w5_pack
    ;;
  *)
    echo "ERROR: unknown subcommand '$SUBCMD'" >&2
    usage
    exit 2
    ;;
esac
