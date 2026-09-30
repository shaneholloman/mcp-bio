#!/usr/bin/env bash
# Decide whether this release's container image should carry the
# `latest` tag (2026-09-30 second go-request review: the old check
# asked GitHub for "the latest release", which SKIPS drafts, so a
# draft v0.9.1 read v0.9.0 as latest and the step silently skipped).
# The decision compares the publishing tag's version number against
# every already-PUBLISHED release: if no published release sorts
# newer, this tag becomes latest once published.
#
# Usage: should-move-latest.sh <publishing-tag> <published-tag>...
# Exit 0 when latest should move; exit 1 (with a reason) when not.
set -euo pipefail

publishing="${1:?usage: should-move-latest.sh <publishing-tag> <published-tag>...}"
shift

version_of() {
    local tag="$1"
    printf '%s' "${tag#v}" | tr '.' ' '
}

version_key() {
    # zero-padded fields so lexicographic sort equals version order
    # for the pre-1.0 shapes this project ships (major.minor.patch).
    local tag="$1"
    printf '%03d.%03d.%03d' $(version_of "$tag")
}

if ! printf '%s' "$publishing" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "not a stable tag: $publishing" >&2
    exit 1
fi

for published in "$@"; do
    if [ "$(version_key "$published")" \> "$(version_key "$publishing")" ]; then
        echo "$published sorts newer than $publishing; latest stays" >&2
        exit 1
    fi
done
exit 0
