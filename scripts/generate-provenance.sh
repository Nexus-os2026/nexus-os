#!/usr/bin/env bash
# generate-provenance.sh — Generate SLSA v1.0 provenance in in-toto Statement format.
#
# Usage:
#   generate-provenance.sh --artifacts <path>... --output <path> [--sbom-ref <path>]

set -euo pipefail

ARTIFACTS=()
OUTPUT="provenance.intoto.jsonl"
SBOM_REF=""
BUILDER_ID=""
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat <<EOF
Usage:
  $0 --artifacts <path> [<path>...] --output <path> [--sbom-ref <path>] [--builder-id <id>]

Options:
  --artifacts    One or more artifact paths to include as subjects
  --output       Output path (default: provenance.intoto.jsonl)
  --sbom-ref     Path to SBOM file to reference in provenance metadata
  --builder-id   Builder identifier (default: from CI or "local-builder")
  -h, --help     Show this help message
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --artifacts)
            shift
            while [[ $# -gt 0 && ! "$1" =~ ^-- ]]; do
                ARTIFACTS+=("$1")
                shift
            done
            ;;
        --output)     OUTPUT="$2"; shift 2 ;;
        --sbom-ref)   SBOM_REF="$2"; shift 2 ;;
        --builder-id) BUILDER_ID="$2"; shift 2 ;;
        -h|--help)    usage ;;
        *)            echo "Error: unknown option '$1'"; usage ;;
    esac
done

if [[ ${#ARTIFACTS[@]} -eq 0 ]]; then
    echo "Error: at least one --artifacts path is required"
    usage
fi

# ---------------------------------------------------------------------------
# Collect build environment
# ---------------------------------------------------------------------------

echo "Collecting build environment..."

OS_INFO=$(uname -s 2>/dev/null || echo "unknown")
ARCH_INFO=$(uname -m 2>/dev/null || echo "unknown")
KERNEL_INFO=$(uname -r 2>/dev/null || echo "unknown")
RUST_VERSION=$(rustc --version 2>/dev/null | awk '{print $2}' || echo "unknown")
NODE_VERSION=$(node --version 2>/dev/null | sed 's/^v//' || echo "")
GIT_COMMIT=$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_TAG=$(git -C "$REPO_ROOT" describe --tags --exact-match 2>/dev/null || echo "")
GIT_BRANCH=$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")

CI_PIPELINE="${CI_PIPELINE_ID:-}"
CI_JOB="${CI_JOB_ID:-}"

if [[ -z "$BUILDER_ID" ]]; then
    if [[ -n "$CI_PIPELINE" ]]; then
        BUILDER_ID="https://gitlab.com/nexaiceo/nexus-os/-/pipelines/${CI_PIPELINE}"
    else
        BUILDER_ID="local-builder"
    fi
fi

INVOCATION_ID=$(cat /proc/sys/kernel/random/uuid 2>/dev/null || python3 -c 'import uuid; print(uuid.uuid4())')
BUILD_STARTED=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# ---------------------------------------------------------------------------
# Compute artifact hashes and build subjects array
# ---------------------------------------------------------------------------

echo "Computing artifact hashes..."

SUBJECTS_JSON="["
MATERIALS_JSON="["
FIRST=true

for artifact in "${ARTIFACTS[@]}"; do
    if [[ ! -f "$artifact" ]]; then
        echo "Warning: artifact not found, skipping: $artifact"
        continue
    fi

    HASH=$(sha256sum "$artifact" | awk '{print $1}')
    NAME=$(basename "$artifact")

    if [[ "$FIRST" != true ]]; then
        SUBJECTS_JSON+=","
        MATERIALS_JSON+=","
    fi
    FIRST=false

    SUBJECTS_JSON+="
    {\"name\":\"$NAME\",\"digest\":{\"sha256\":\"$HASH\"}}"
    MATERIALS_JSON+="
    {\"uri\":\"file://$artifact\",\"digest\":{\"sha256\":\"$HASH\"},\"name\":\"$NAME\"}"

    echo "  $NAME: $HASH"
done

SUBJECTS_JSON+="
  ]"
MATERIALS_JSON+="
  ]"

# Add Cargo.lock and package-lock.json as materials
CARGO_LOCK="$REPO_ROOT/Cargo.lock"
if [[ -f "$CARGO_LOCK" ]]; then
    LOCK_HASH=$(sha256sum "$CARGO_LOCK" | awk '{print $1}')
    MATERIALS_JSON="${MATERIALS_JSON%]},
    {\"uri\":\"file://Cargo.lock\",\"digest\":{\"sha256\":\"$LOCK_HASH\"},\"name\":\"Cargo.lock\"}
  ]"
fi

NPM_LOCK="$REPO_ROOT/app/package-lock.json"
if [[ -f "$NPM_LOCK" ]]; then
    NPM_HASH=$(sha256sum "$NPM_LOCK" | awk '{print $1}')
    MATERIALS_JSON="${MATERIALS_JSON%]},
    {\"uri\":\"file://app/package-lock.json\",\"digest\":{\"sha256\":\"$NPM_HASH\"},\"name\":\"package-lock.json\"}
  ]"
fi

# ---------------------------------------------------------------------------
# Build SBOM reference metadata
# ---------------------------------------------------------------------------

SBOM_REF_JSON="null"
if [[ -n "$SBOM_REF" && -f "$SBOM_REF" ]]; then
    SBOM_REF_JSON="\"$SBOM_REF\""
fi

# ---------------------------------------------------------------------------
# Collect cargo features (from workspace Cargo.toml)
# ---------------------------------------------------------------------------

CARGO_FEATURES="[]"

# ---------------------------------------------------------------------------
# Generate SLSA provenance as in-toto Statement
# ---------------------------------------------------------------------------

BUILD_FINISHED=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "Generating SLSA provenance..."

# Node version may be empty
NODE_VER_JSON="null"
if [[ -n "$NODE_VERSION" ]]; then
    NODE_VER_JSON="\"$NODE_VERSION\""
fi

CI_PIPELINE_JSON="null"
if [[ -n "$CI_PIPELINE" ]]; then
    CI_PIPELINE_JSON="\"$CI_PIPELINE\""
fi

CI_JOB_JSON="null"
if [[ -n "$CI_JOB" ]]; then
    CI_JOB_JSON="\"$CI_JOB\""
fi

GIT_TAG_JSON="null"
if [[ -n "$GIT_TAG" ]]; then
    GIT_TAG_JSON="\"$GIT_TAG\""
fi

GIT_BRANCH_JSON="null"
if [[ -n "$GIT_BRANCH" ]]; then
    GIT_BRANCH_JSON="\"$GIT_BRANCH\""
fi

cat > "$OUTPUT" <<PROVENANCE_EOF
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": $SUBJECTS_JSON,
  "predicateType": "https://slsa.dev/provenance/v1",
  "predicate": {
    "buildDefinition": {
      "buildType": "https://nexus-os.dev/build/v1",
      "externalParameters": {
        "git_commit": "$GIT_COMMIT",
        "git_tag": $GIT_TAG_JSON,
        "git_branch": $GIT_BRANCH_JSON,
        "cargo_features": $CARGO_FEATURES
      },
      "internalParameters": {
        "os": "$OS_INFO",
        "arch": "$ARCH_INFO",
        "kernel": "$KERNEL_INFO",
        "rust_version": "$RUST_VERSION",
        "node_version": $NODE_VER_JSON,
        "ci_pipeline_id": $CI_PIPELINE_JSON,
        "ci_job_id": $CI_JOB_JSON
      },
      "resolvedDependencies": $MATERIALS_JSON
    },
    "runDetails": {
      "builder": {
        "id": "$BUILDER_ID"
      },
      "metadata": {
        "invocationId": "$INVOCATION_ID",
        "startedOn": "$BUILD_STARTED",
        "finishedOn": "$BUILD_FINISHED"
      },
      "byproducts": {
        "sbom_reference": $SBOM_REF_JSON,
        "slsa_level": 2,
        "reproducible": false
      }
    }
  }
}
PROVENANCE_EOF

echo "Provenance written to: $OUTPUT"
echo "  Builder:     $BUILDER_ID"
echo "  Git commit:  $GIT_COMMIT"
echo "  Git tag:     ${GIT_TAG:-none}"
echo "  Artifacts:   ${#ARTIFACTS[@]}"
echo "  SLSA level:  2"
echo "Done."
