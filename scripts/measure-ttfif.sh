#!/usr/bin/env bash
# measure-ttfif.sh — clean-room TTFIF (time-to-first-initialized-fleet) capture
# for the add-setup-wizard-onboarding measurement gate (openspec change
# add-setup-wizard-onboarding, ticket DDL-g6w).
#
# Usage:
#   scripts/measure-ttfif.sh baseline          # times `ddl init --yes --human`
#   scripts/measure-ttfif.sh baseline --no-install
#   scripts/measure-ttfif.sh wizard            # after §2 implementation exists
#
# Prints one line per run:
#   ttfif_seconds=<N.N> mode=<mode> tools_healthy=<n>/<m>
# Appends the same line to openspec/changes/add-setup-wizard-onboarding/baseline.org
#
# Environment: builds a fresh temp $HOME + temp git repo, minimal PATH +
# the fleet-pinned toolset if already present in ~/.cargo/bin and the npm
# global dir. Network installs are part of the flow (the real new-user path).
set -euo pipefail

mode="${1:?usage: measure-ttfif.sh baseline|wizard [--no-install]}"
shift || true
extra_flags=""
for arg in "$@"; do
  case "$arg" in
    --no-install) extra_flags="$extra_flags --no-install" ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
ddl_bin="$repo_root/target/release/ddl"
[ -x "$ddl_bin" ] || ddl_bin="$(command -v ddl)"
[ -x "$ddl_bin" ] || { echo "no ddl binary (build with: cargo build --release)" >&2; exit 2; }

results="$repo_root/openspec/changes/add-setup-wizard-onboarding/baseline.org"
mkdir -p "$(dirname "$results")"

tmp_home="$(mktemp -d)"
tmp_repo="$(mktemp -d)"
trap 'rm -rf "$tmp_home" "$tmp_repo"' EXIT

git -C "$tmp_repo" init -q
echo "# stock repo files a new user would have" > "$tmp_repo/README.md"

export HOME="$tmp_home"
# True new-user PATH: system dirs only. Fleet tools are NOT on PATH here —
# a real new user has none, and preinstalling some (brew/cargo) would bias
# the baseline down (CORR-002, rule-of-5 review of this proposal). The
# pinned-toolset PATH variant belongs in CI, not in this harness.
export PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
# Optionally expose a pinned toolset for subset runs:
#   TTFIF_TOOL_PATH=/path/to/pinned/bin scripts/measure-ttfif.sh baseline
if [ -n "${TTFIF_TOOL_PATH:-}" ]; then
  export PATH="$TTFIF_TOOL_PATH:$PATH"
fi

first_cmd="$ddl_bin init --yes --human$extra_flags"
start="$(date +%s.%N)"
if [ "$mode" = "wizard" ]; then
  # Post-implementation: the wizard orchestrates; init delegates to it.
  first_cmd="$ddl_bin setup --yes"
fi
set +e
out="$(cd "$tmp_repo" && $first_cmd 2>&1)"
init_rc=$?
set -e

# Harness fix (DDL-g6w.6): init installs into $HOME/.ddl/bin but status probes
# PATH — a real user's post-init shell has the bin dir available, so expose it
# for the status probe (and ONLY for the probe, not for the first command).
status_path="$HOME/.ddl/bin:$PATH"
status_out="$(cd "$tmp_repo" && env PATH="$status_path" "$ddl_bin" status --json 2>/dev/null || true)"
# grep exits 1 on zero matches — with pipefail+set -e that kills the script
# exactly when a run has 0 healthy tools (the true new-user case). Guard it.
healthy="$(echo "$status_out" | { grep -o '"level": *"healthy"' || true; } | wc -l)"
end="$(date +%s.%N)"
ttfif="$(awk -v a="$end" -v b="$start" 'BEGIN{printf "%.3f", a-b}')"
[ -n "${TTFIF_DEBUG:-}" ] && echo "DBG start=$start end=$end init_rc=$init_rc out_head=$(echo "$out" | head -1)" >&2

line="ttfif_seconds=$ttfif mode=$mode tools_healthy=${healthy} init_rc=$init_rc date=$(date -Iseconds) flags='${extra_flags:-none}'"
echo "$line"
echo "- $line" >> "$results"
[ "$init_rc" -eq 0 ] || { echo "init exited $init_rc — output tail:" >&2; echo "$out" | tail -5 >&2; }
