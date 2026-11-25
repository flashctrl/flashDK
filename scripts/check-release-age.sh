#!/usr/bin/env bash
# Check that dependency versions adopted since a base ref have aged past the cooldown:
# 7 days, or 14 for a major version (for 0.x crates a minor bump is a major one).
# Security fixes are exempt and must be listed in scripts/release-age-exempt.txt.
#
# Usage: scripts/check-release-age.sh [base-ref]      (default: origin/main)
# Needs bash 3.2+, git, curl and jq. Publish times come from the crates.io index.
set -euo pipefail

base="${1:-origin/main}"
root="$(git rev-parse --show-toplevel)"
exempt="$root/scripts/release-age-exempt.txt"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# "name version" for every registry package in a Cargo.lock.
pairs() {
    awk '/^\[\[package\]\]/ { n=""; v="" }
         /^name = /    { gsub(/"/, "", $3); n=$3 }
         /^version = / { gsub(/"/, "", $3); v=$3 }
         /^source = "registry/ { print n, v }' | sort -u
}

git -C "$root" show "$base:Cargo.lock" | pairs >"$tmp/base"
pairs <"$root/Cargo.lock" >"$tmp/head"
comm -13 "$tmp/base" "$tmp/head" >"$tmp/new"

# The semver-compatible line of a version: "1" for 1.x.y, "0.24" for 0.24.y.
line_of() {
    printf '%s\n' "$1" | awk -F. '{ if ($1 != "0") print $1; else print "0." $2 }'
}

index_path() {
    local n
    n="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
    case ${#n} in
    1) printf '1/%s' "$n" ;;
    2) printf '2/%s' "$n" ;;
    3) printf '3/%s/%s' "${n:0:1}" "$n" ;;
    *) printf '%s/%s/%s' "${n:0:2}" "${n:2:2}" "$n" ;;
    esac
}

now="$(date -u +%s)"
fail=0
checked=0
while read -r name version; do
    wait_days=7
    # A crate already present only on other compatible lines is a major upgrade.
    if grep -q "^$name " "$tmp/base"; then
        wait_days=14
        while read -r _ old; do
            [ "$(line_of "$old")" = "$(line_of "$version")" ] && wait_days=7
        done < <(grep "^$name " "$tmp/base")
    fi

    published="$(curl -fsS -m 20 -A 'dependency-age-check' \
        "https://index.crates.io/$(index_path "$name")" |
        jq -r --arg v "$version" 'select(.vers == $v) | .pubtime // empty')"
    if [ -z "$published" ]; then
        echo "UNKNOWN  $name $version has no publish time; check it by hand"
        fail=1
        continue
    fi
    checked=$((checked + 1))

    age=$(((now - $(jq -rn --arg t "$published" '$t | fromdateiso8601')) / 86400))
    [ "$age" -ge "$wait_days" ] && continue

    if [ -f "$exempt" ] && grep -q "^$name $version " "$exempt"; then
        echo "exempt   $name $version ($age days): $(grep "^$name $version " "$exempt" | cut -d' ' -f3-)"
    else
        echo "TOO NEW  $name $version is $age days old, needs $wait_days (published $published)"
        fail=1
    fi
done <"$tmp/new"

echo "checked $checked new versions against $base"
exit "$fail"
