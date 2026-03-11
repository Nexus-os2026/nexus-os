#!/usr/bin/env bash
# sign-release.sh — Sign release artifacts using cosign (Sigstore keyless signing).
#
# Usage:
#   sign-release.sh --artifact <path> --output-dir <dir> [--dry-run]
#   sign-release.sh --verify --artifact <path> --output-dir <dir>

set -euo pipefail

ARTIFACT=""
OUTPUT_DIR=""
VERIFY=false
DRY_RUN=false

usage() {
    cat <<EOF
Usage:
  $0 --artifact <path> --output-dir <dir> [--dry-run]
  $0 --verify --artifact <path> --output-dir <dir>

Options:
  --artifact     Path to the artifact to sign or verify
  --output-dir   Directory for signature and certificate files
  --verify       Verify mode: check existing signature and certificate
  --dry-run      Skip actual cosign calls; generate manifest structure only
  -h, --help     Show this help message
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --artifact)   ARTIFACT="$2"; shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        --verify)     VERIFY=true; shift ;;
        --dry-run)    DRY_RUN=true; shift ;;
        -h|--help)    usage ;;
        *)            echo "Error: unknown option '$1'"; usage ;;
    esac
done

if [[ -z "$ARTIFACT" || -z "$OUTPUT_DIR" ]]; then
    echo "Error: --artifact and --output-dir are required"
    usage
fi

if [[ ! -f "$ARTIFACT" ]]; then
    echo "Error: artifact not found: $ARTIFACT"
    exit 1
fi

FILENAME=$(basename "$ARTIFACT")
SIG_FILE="${OUTPUT_DIR}/${FILENAME}.sig"
CRT_FILE="${OUTPUT_DIR}/${FILENAME}.crt"

compute_sha256() {
    sha256sum "$1" | awk '{print $1}'
}

# --- Verify mode ---
if [[ "$VERIFY" == true ]]; then
    echo "Verifying artifact: $ARTIFACT"

    HASH=$(compute_sha256 "$ARTIFACT")
    echo "SHA-256: $HASH"

    if [[ ! -f "$SIG_FILE" ]]; then
        echo "Error: signature file not found: $SIG_FILE"
        exit 1
    fi
    if [[ ! -f "$CRT_FILE" ]]; then
        echo "Error: certificate file not found: $CRT_FILE"
        exit 1
    fi

    if [[ "$DRY_RUN" == true ]]; then
        echo "[dry-run] Would run: cosign verify-blob --signature $SIG_FILE --certificate $CRT_FILE --certificate-identity-regexp '.' --certificate-oidc-issuer-regexp '.' $ARTIFACT"
        echo "Verification: skipped (dry-run)"
    else
        cosign verify-blob \
            --signature "$SIG_FILE" \
            --certificate "$CRT_FILE" \
            --certificate-identity-regexp '.' \
            --certificate-oidc-issuer-regexp '.' \
            "$ARTIFACT"
        echo "Verification: passed"
    fi
    exit 0
fi

# --- Sign mode ---
mkdir -p "$OUTPUT_DIR"

echo "Signing artifact: $ARTIFACT"
HASH=$(compute_sha256 "$ARTIFACT")
echo "SHA-256: $HASH"

if [[ "$DRY_RUN" == true ]]; then
    echo "[dry-run] Would run: cosign sign-blob --yes --output-signature $SIG_FILE --output-certificate $CRT_FILE $ARTIFACT"
    # Create placeholder files for manifest generation
    echo "DRY_RUN_SIGNATURE" > "$SIG_FILE"
    echo "DRY_RUN_CERTIFICATE" > "$CRT_FILE"
else
    cosign sign-blob \
        --yes \
        --output-signature "$SIG_FILE" \
        --output-certificate "$CRT_FILE" \
        "$ARTIFACT"
fi

echo "Signature:   $SIG_FILE"
echo "Certificate: $CRT_FILE"

# Generate signing manifest JSON
TIMESTAMP=$(date +%s)
GIT_COMMIT="${CI_COMMIT_SHA:-$(git rev-parse HEAD 2>/dev/null || echo 'unknown')}"
GIT_TAG="${CI_COMMIT_TAG:-$(git describe --tags --exact-match 2>/dev/null || echo 'untagged')}"
PIPELINE_ID="${CI_PIPELINE_ID:-local}"

MANIFEST_FILE="${OUTPUT_DIR}/${FILENAME}.manifest.json"

cat > "$MANIFEST_FILE" <<MANIFEST_EOF
{
  "version": "1.0.0",
  "artifacts": [
    {
      "artifact_path": "$ARTIFACT",
      "artifact_hash": "$HASH",
      "signature_path": "$SIG_FILE",
      "certificate_path": "$CRT_FILE",
      "timestamp": $TIMESTAMP,
      "signer_identity": "${COSIGN_IDENTITY:-ci-pipeline}",
      "dry_run": $DRY_RUN
    }
  ],
  "build_id": "${CI_JOB_ID:-local-$(date +%s)}",
  "git_commit": "$GIT_COMMIT",
  "git_tag": "$GIT_TAG",
  "pipeline_id": "$PIPELINE_ID",
  "created_at": $TIMESTAMP
}
MANIFEST_EOF

echo "Manifest:    $MANIFEST_FILE"
echo "Done."
