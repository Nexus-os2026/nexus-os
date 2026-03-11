#!/usr/bin/env bash
# verify-release.sh — Verify all artifacts listed in a signing manifest.
#
# Usage:
#   verify-release.sh <manifest.json> [--dry-run]
#
# For each artifact in the manifest:
#   1. Verify SHA-256 hash matches the file on disk.
#   2. If .sig and .crt files exist, verify cosign signature.

set -euo pipefail

DRY_RUN=false
MANIFEST=""

usage() {
    cat <<EOF
Usage:
  $0 <manifest.json> [--dry-run]

Options:
  --dry-run   Skip actual cosign verify-blob calls
  -h, --help  Show this help message
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)   DRY_RUN=true; shift ;;
        -h|--help)   usage ;;
        -*)          echo "Error: unknown option '$1'"; usage ;;
        *)
            if [[ -z "$MANIFEST" ]]; then
                MANIFEST="$1"; shift
            else
                echo "Error: unexpected argument '$1'"; usage
            fi
            ;;
    esac
done

if [[ -z "$MANIFEST" ]]; then
    echo "Error: manifest file is required"
    usage
fi

if [[ ! -f "$MANIFEST" ]]; then
    echo "Error: manifest not found: $MANIFEST"
    exit 1
fi

if ! command -v jq &>/dev/null; then
    echo "Error: jq is required but not installed"
    exit 1
fi

TOTAL=0
PASSED=0
FAILED=0
FAILURES=""

ARTIFACT_COUNT=$(jq '.artifacts | length' "$MANIFEST")

if [[ "$ARTIFACT_COUNT" -eq 0 ]]; then
    echo "Error: manifest contains no artifacts"
    exit 1
fi

echo "Verifying $ARTIFACT_COUNT artifact(s) from manifest: $MANIFEST"
echo "Git commit: $(jq -r '.git_commit' "$MANIFEST")"
echo "Git tag:    $(jq -r '.git_tag' "$MANIFEST")"
echo "Pipeline:   $(jq -r '.pipeline_id' "$MANIFEST")"
echo "---"

for i in $(seq 0 $((ARTIFACT_COUNT - 1))); do
    ARTIFACT_PATH=$(jq -r ".artifacts[$i].artifact_path" "$MANIFEST")
    EXPECTED_HASH=$(jq -r ".artifacts[$i].artifact_hash" "$MANIFEST")
    SIG_PATH=$(jq -r ".artifacts[$i].signature_path // empty" "$MANIFEST")
    CRT_PATH=$(jq -r ".artifacts[$i].certificate_path // empty" "$MANIFEST")

    TOTAL=$((TOTAL + 1))
    echo "[$((i + 1))/$ARTIFACT_COUNT] $ARTIFACT_PATH"

    # Check file exists
    if [[ ! -f "$ARTIFACT_PATH" ]]; then
        echo "  FAIL: file not found"
        FAILED=$((FAILED + 1))
        FAILURES="${FAILURES}\n  - ${ARTIFACT_PATH}: file not found"
        continue
    fi

    # Verify SHA-256 hash
    ACTUAL_HASH=$(sha256sum "$ARTIFACT_PATH" | awk '{print $1}')
    if [[ "$ACTUAL_HASH" != "$EXPECTED_HASH" ]]; then
        echo "  FAIL: hash mismatch"
        echo "    expected: $EXPECTED_HASH"
        echo "    actual:   $ACTUAL_HASH"
        FAILED=$((FAILED + 1))
        FAILURES="${FAILURES}\n  - ${ARTIFACT_PATH}: hash mismatch"
        continue
    fi
    echo "  SHA-256: OK"

    # Verify cosign signature if .sig and .crt files exist
    if [[ -n "$SIG_PATH" && -f "$SIG_PATH" && -n "$CRT_PATH" && -f "$CRT_PATH" ]]; then
        if [[ "$DRY_RUN" == true ]]; then
            echo "  [dry-run] Would verify cosign signature"
            echo "  Signature: OK (dry-run)"
        else
            if cosign verify-blob \
                --signature "$SIG_PATH" \
                --certificate "$CRT_PATH" \
                --certificate-identity-regexp '.' \
                --certificate-oidc-issuer-regexp '.' \
                "$ARTIFACT_PATH" 2>/dev/null; then
                echo "  Signature: OK"
            else
                echo "  FAIL: cosign signature verification failed"
                FAILED=$((FAILED + 1))
                FAILURES="${FAILURES}\n  - ${ARTIFACT_PATH}: signature verification failed"
                continue
            fi
        fi
    else
        echo "  Signature: skipped (no .sig/.crt files)"
    fi

    PASSED=$((PASSED + 1))
done

echo "---"
echo "Results: $PASSED passed, $FAILED failed, $TOTAL total"

if [[ "$FAILED" -gt 0 ]]; then
    echo -e "Failures:$FAILURES"
    exit 1
fi

echo "All artifacts verified."
exit 0
