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
# Realistic new-user PATH: system dirs + wherever the pinned fleet already
# lives (cargo bin + brew + npm global). NOT the repo's .ddl/bin.
export PATH="/usr/local/bin:/usr/bin:/bin:$HOME/.cargo/bin:/home/linuxbrew/.linuxbrew/bin"

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

status_out="$(cd "$tmp_repo" && "$ddl_bin" status --json 2>/dev/null || true)"
healthy="$(echo "$status_out" | grep -o '"level": *"healthy"' | wc -l)"
end="$(date +%s.%N)"
ttfif="$(echo "$end - $start" | bc)"

line="ttfif_seconds=$ttfif mode=$mode tools_healthy=${healthy} init_rc=$init_rc date=$(date -Iseconds) flags='${extra_flags:-none}'"
echo "$line"
echo "- $line" >> "$results"
[ "$init_rc" -eq 0 ] || { echo "init exited $init_rc — output tail:" >&2; echo "$out" | tail -5 >&2; }
