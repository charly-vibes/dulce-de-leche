#!/usr/bin/env bash
# Purpose: executable fixture tests for the stable `install.sh` bootstrap
#          entry point (DDL-1ay / SEC-03).
# Responsibilities:
#   - Prove the SEC-03 musts at shell level: a clean fixture installs an
#     executable `ddl`; a tampered archive and a missing/unusable
#     checksums.txt cause a non-zero exit and ZERO executable installs.
#   - Prove DDL-yts policy parity: fail-closed by default, with the same
#     `DDL_ALLOW_UNVERIFIED_RELEASE=1` explicit-authorization escape hatch.
# Rationale: the curl snippet previously piped the release archive straight
#   into tar/exec without any verification; these fixtures pin the
#   verify-before-extract contract so regressions fail CI.
#
# No network access: fixtures are served from the local filesystem via
# `file://` base URLs (install.sh accepts DDL_BASE_URL/DDL_VERSION/DDL_INSTALL_DIR).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INSTALL_SH="$ROOT/install.sh"

PASS=0
FAIL=0

# Build a release fixture directory containing $archive_name (a tar.gz whose
# only member is an executable `ddl` stub) and a checksums.txt ($2: "ok" or
# "corrupt" digest, $3: whether to write checksums.txt at all).
make_fixture() {
    local dir="$1" archive_name="$2" mode="$3" with_checksums="$4"
    mkdir -p "$dir/payload" "$dir/dest"
    cat > "$dir/payload/ddl" <<'EOF'
#!/usr/bin/env bash
echo "ddl-stub 1.2.3"
EOF
    chmod +x "$dir/payload/ddl"
    tar -czf "$dir/$archive_name" -C "$dir/payload" ddl
    if [[ "$with_checksums" == "yes" ]]; then
        if [[ "$mode" == "ok" ]]; then
            local digest
            digest="$(sha256sum "$dir/$archive_name" | awk '{print $1}')"
            printf '%s  %s\n' "$digest" "$archive_name" > "$dir/checksums.txt"
        elif [[ "$mode" == "tampered" ]]; then
            # digest for the ORIGINAL bytes is published, then the archive is
            # mutated afterwards -> verifier must reject the mismatch.
            local orig
            orig="$dir/orig.tar.gz"
            cp -f "$dir/$archive_name" "$orig"
            printf '%s  %s\n' "$(sha256sum "$orig" | awk '{print $1}')" "$archive_name" \
                > "$dir/checksums.txt"
            printf 'TAMPERED' >> "$dir/$archive_name"
        fi
    fi
}

# run_install <fixture_dir> <dest_dir> [extra env assignments...]
run_install() {
    local fixture="$1" dest="$2"
    shift 2
    DDL_VERSION="1.2.3" \
        DDL_BASE_URL="file://$fixture" \
        DDL_INSTALL_DIR="$dest" \
        "$@" \
        bash "$INSTALL_SH" >/dev/null 2>&1
}

assert_no_executable_installed() {
    local dest="$1" label="$2"
    if [[ -e "$dest/ddl" ]]; then
        echo "FAIL: $label — executable was installed into $dest"
        FAIL=$((FAIL + 1))
    else
        echo "ok:   $label — zero executable installs"
        PASS=$((PASS + 1))
    fi
}

require_install_sh() {
    if [[ ! -f "$INSTALL_SH" ]]; then
        echo "FAIL: $INSTALL_SH does not exist yet (red phase expected)"
        exit 1
    fi
}

# --- SEC-03 Must 1: clean fixture succeeds and installs an executable -------
test_clean_fixture_succeeds() {
    local f; f="$(mktemp -d)"
    make_fixture "$f" "ddl_1.2.3_linux_amd64.tar.gz" ok yes
    if run_install "$f" "$f/dest"; then
        if [[ -x "$f/dest/ddl" && "$("$f/dest/ddl")" == "ddl-stub 1.2.3" ]]; then
            echo "ok:   clean fixture installs verified executable"
            PASS=$((PASS + 1))
        else
            echo "FAIL: clean fixture — installed ddl missing or not executable"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL: clean fixture — install.sh rejected a valid verified release"
        FAIL=$((FAIL + 1))
    fi
    rm -rf "$f"
}

# --- SEC-03 Must 2: tampered archive -> reject, zero executable installs ----
test_tampered_archive_rejected() {
    local f; f="$(mktemp -d)"
    make_fixture "$f" "ddl_1.2.3_linux_amd64.tar.gz" tampered yes
    if run_install "$f" "$f/dest"; then
        echo "FAIL: tampered fixture — install.sh accepted a checksum mismatch"
        FAIL=$((FAIL + 1))
    else
        assert_no_executable_installed "$f/dest" "tampered archive"
    fi
    rm -rf "$f"
}

# --- SEC-03 Must 3: missing checksums.txt -> reject, zero installs ----------
test_missing_checksums_rejected() {
    local f; f="$(mktemp -d)"
    make_fixture "$f" "ddl_1.2.3_linux_amd64.tar.gz" ok no
    if run_install "$f" "$f/dest"; then
        echo "FAIL: missing-checksum fixture — install.sh installed unverified"
        FAIL=$((FAIL + 1))
    else
        assert_no_executable_installed "$f/dest" "missing checksums.txt"
    fi
    rm -rf "$f"
}

# --- SEC-03 Must 3b: asset not listed in checksums.txt -> reject ------------
test_unlisted_asset_rejected() {
    local f; f="$(mktemp -d)"
    make_fixture "$f" "ddl_1.2.3_linux_amd64.tar.gz" ok yes
    printf '%s  other_asset.tar.gz\n' \
        "$(sha256sum "$f/ddl_1.2.3_linux_amd64.tar.gz" | awk '{print $1}')" \
        > "$f/checksums.txt"
    if run_install "$f" "$f/dest"; then
        echo "FAIL: unlisted-asset fixture — install.sh installed unverified"
        FAIL=$((FAIL + 1))
    else
        assert_no_executable_installed "$f/dest" "asset unlisted in checksums.txt"
    fi
    rm -rf "$f"
}

# --- DDL-yts parity: explicit authorization is the only bypass --------------
test_allow_unverified_authorized() {
    local f; f="$(mktemp -d)"
    make_fixture "$f" "ddl_1.2.3_linux_amd64.tar.gz" ok no
    if run_install "$f" "$f/dest" DDL_ALLOW_UNVERIFIED_RELEASE=1; then
        if [[ -x "$f/dest/ddl" ]]; then
            echo "ok:   DDL_ALLOW_UNVERIFIED_RELEASE=1 authorizes missing checksums"
            PASS=$((PASS + 1))
        else
            echo "FAIL: allow-unverified fixture — no install despite authorization"
            FAIL=$((FAIL + 1))
        fi
    else
        echo "FAIL: allow-unverified fixture — explicit authorization not honored"
        FAIL=$((FAIL + 1))
    fi
    rm -rf "$f"
}

require_install_sh
test_clean_fixture_succeeds
test_tampered_archive_rejected
test_missing_checksums_rejected
test_unlisted_asset_rejected
test_allow_unverified_authorized

echo "----------------------------------------"
echo "install.sh fixtures: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
