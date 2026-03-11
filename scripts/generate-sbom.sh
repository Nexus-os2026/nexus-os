#!/usr/bin/env bash
# generate-sbom.sh — Generate CycloneDX SBOM from Cargo.lock and package-lock.json.
#
# Usage:
#   generate-sbom.sh [--output <path>] [--verify <sbom-path>] [--project-version <ver>]

set -euo pipefail

OUTPUT="sbom.cdx.json"
VERIFY=""
PROJECT_VERSION=""
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat <<EOF
Usage:
  $0 [--output <path>] [--project-version <ver>]
  $0 --verify <sbom-path>

Options:
  --output            Output path for the SBOM (default: sbom.cdx.json)
  --project-version   Project version string (default: read from Cargo.toml)
  --verify            Verify mode: check SBOM against current lock files
  -h, --help          Show this help message
EOF
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)          OUTPUT="$2"; shift 2 ;;
        --verify)          VERIFY="$2"; shift 2 ;;
        --project-version) PROJECT_VERSION="$2"; shift 2 ;;
        -h|--help)         usage ;;
        *)                 echo "Error: unknown option '$1'"; usage ;;
    esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

compute_sha256() {
    sha256sum "$1" | awk '{print $1}'
}

# Read project version from workspace Cargo.toml if not provided
if [[ -z "$PROJECT_VERSION" ]]; then
    PROJECT_VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
fi

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
SERIAL="urn:uuid:$(cat /proc/sys/kernel/random/uuid 2>/dev/null || python3 -c 'import uuid; print(uuid.uuid4())')"

# ---------------------------------------------------------------------------
# Parse Cargo.lock into JSON components
# ---------------------------------------------------------------------------

parse_cargo_lock() {
    local cargo_lock="$REPO_ROOT/Cargo.lock"
    if [[ ! -f "$cargo_lock" ]]; then
        echo "Error: Cargo.lock not found at $cargo_lock" >&2
        exit 1
    fi

    # Use awk to extract [[package]] entries
    awk '
    BEGIN { first = 1; print "[" }
    /^\[\[package\]\]/ { in_pkg = 1; name = ""; version = ""; source = ""; next }
    in_pkg && /^name = / {
        gsub(/^name = "/, ""); gsub(/"$/, ""); name = $0; next
    }
    in_pkg && /^version = / {
        gsub(/^version = "/, ""); gsub(/"$/, ""); version = $0; next
    }
    in_pkg && /^source = / {
        gsub(/^source = "/, ""); gsub(/"$/, ""); source = $0; next
    }
    in_pkg && /^$/ {
        if (name != "" && version != "") {
            if (!first) printf ","
            first = 0
            # Determine if workspace crate (no source) or external
            if (source == "") {
                etype = "WorkspaceCrate"
            } else {
                etype = "RustCrate"
            }
            printf "\n  {\"type\":\"library\",\"name\":\"%s\",\"version\":\"%s\",\"purl\":\"pkg:cargo/%s@%s\",\"bom-ref\":\"cargo-%s-%s\",\"properties\":[{\"name\":\"nexus:entry_type\",\"value\":\"%s\"}]}", name, version, name, version, name, version, etype
        }
        in_pkg = 0
    }
    END {
        # Flush last package if file does not end with blank line
        if (in_pkg && name != "" && version != "") {
            if (!first) printf ","
            if (source == "") etype = "WorkspaceCrate"; else etype = "RustCrate"
            printf "\n  {\"type\":\"library\",\"name\":\"%s\",\"version\":\"%s\",\"purl\":\"pkg:cargo/%s@%s\",\"bom-ref\":\"cargo-%s-%s\",\"properties\":[{\"name\":\"nexus:entry_type\",\"value\":\"%s\"}]}", name, version, name, version, name, version, etype
        }
        print "\n]"
    }
    ' "$cargo_lock"
}

# ---------------------------------------------------------------------------
# Parse package-lock.json into JSON components
# ---------------------------------------------------------------------------

parse_npm_lock() {
    local lock_file="$REPO_ROOT/app/package-lock.json"
    if [[ ! -f "$lock_file" ]]; then
        echo "[]"
        return
    fi

    # Use node if available, otherwise python3, otherwise jq
    if command -v node &>/dev/null; then
        node -e "
const lock = require('$lock_file');
const pkgs = lock.packages || {};
const components = [];
for (const [path, info] of Object.entries(pkgs)) {
    if (!path || path === '') continue; // skip root
    const name = path.replace(/^node_modules\//, '');
    if (!info.version) continue;
    components.push({
        type: 'library',
        name: name,
        version: info.version,
        purl: 'pkg:npm/' + encodeURIComponent(name) + '@' + info.version,
        'bom-ref': 'npm-' + name.replace(/[\/@]/g, '-') + '-' + info.version,
        properties: [{name: 'nexus:entry_type', value: 'NpmPackage'}]
    });
}
console.log(JSON.stringify(components, null, 2));
"
    elif command -v python3 &>/dev/null; then
        python3 -c "
import json, urllib.parse, re, sys
with open('$lock_file') as f:
    lock = json.load(f)
pkgs = lock.get('packages', {})
components = []
for path, info in pkgs.items():
    if not path:
        continue
    name = re.sub(r'^node_modules/', '', path)
    version = info.get('version', '')
    if not version:
        continue
    components.append({
        'type': 'library',
        'name': name,
        'version': version,
        'purl': f'pkg:npm/{urllib.parse.quote(name, safe=\"\")}@{version}',
        'bom-ref': f'npm-{re.sub(r\"[/@]\", \"-\", name)}-{version}',
        'properties': [{'name': 'nexus:entry_type', 'value': 'NpmPackage'}]
    })
json.dump(components, sys.stdout, indent=2)
print()
"
    else
        echo "Warning: neither node nor python3 found, skipping npm SBOM" >&2
        echo "[]"
    fi
}

# ---------------------------------------------------------------------------
# Verify mode
# ---------------------------------------------------------------------------

if [[ -n "$VERIFY" ]]; then
    if [[ ! -f "$VERIFY" ]]; then
        echo "Error: SBOM file not found: $VERIFY"
        exit 1
    fi

    echo "Verifying SBOM: $VERIFY"
    ERRORS=0

    # Extract component names+versions from the SBOM
    SBOM_COMPONENTS=$(python3 -c "
import json, sys
with open('$VERIFY') as f:
    sbom = json.load(f)
for c in sbom.get('components', []):
    props = {p['name']: p['value'] for p in c.get('properties', [])}
    entry_type = props.get('nexus:entry_type', '')
    print(f\"{entry_type}|{c['name']}|{c['version']}\")
" 2>/dev/null || echo "PARSE_ERROR")

    if [[ "$SBOM_COMPONENTS" == "PARSE_ERROR" ]]; then
        echo "Error: failed to parse SBOM JSON"
        exit 1
    fi

    # Verify all components against lock files using python
    CARGO_LOCK="$REPO_ROOT/Cargo.lock"
    NPM_LOCK="$REPO_ROOT/app/package-lock.json"

    VERIFY_RESULT=$(python3 - "$VERIFY" "$CARGO_LOCK" "$NPM_LOCK" <<'PYEOF'
import json, re, sys

sbom_path, cargo_lock_path, npm_lock_path = sys.argv[1:4]
errors = 0

with open(sbom_path) as f:
    sbom = json.load(f)

# Build set of (name, version) from Cargo.lock
cargo_pkgs = set()
try:
    with open(cargo_lock_path) as f:
        name = version = ""
        for line in f:
            line = line.rstrip()
            if line == "[[package]]":
                if name and version:
                    cargo_pkgs.add((name, version))
                name = version = ""
            elif line.startswith('name = "'):
                name = line.split('"')[1]
            elif line.startswith('version = "'):
                version = line.split('"')[1]
        if name and version:
            cargo_pkgs.add((name, version))
except FileNotFoundError:
    pass

# Build set of (name, version) from package-lock.json
npm_pkgs = set()
try:
    with open(npm_lock_path) as f:
        lock = json.load(f)
    for path, info in lock.get("packages", {}).items():
        if not path:
            continue
        pname = re.sub(r"^node_modules/", "", path)
        pver = info.get("version", "")
        if pver:
            npm_pkgs.add((pname, pver))
except FileNotFoundError:
    pass

for c in sbom.get("components", []):
    props = {p["name"]: p["value"] for p in c.get("properties", [])}
    etype = props.get("nexus:entry_type", "")
    name = c["name"]
    version = c["version"]

    if etype in ("RustCrate", "WorkspaceCrate"):
        if (name, version) not in cargo_pkgs:
            print(f"  MISSING in Cargo.lock: {name}@{version}")
            errors += 1
    elif etype == "NpmPackage":
        if (name, version) not in npm_pkgs:
            print(f"  MISSING in package-lock.json: {name}@{version}")
            errors += 1

print(f"ERRORS:{errors}")
PYEOF
    )

    echo "$VERIFY_RESULT" | grep -v '^ERRORS:' || true
    ERRORS=$(echo "$VERIFY_RESULT" | grep '^ERRORS:' | cut -d: -f2)

    if [[ $ERRORS -eq 0 ]]; then
        echo "Verification: passed — all SBOM components match lock files"
        exit 0
    else
        echo "Verification: FAILED — $ERRORS component(s) not found in lock files"
        exit 1
    fi
fi

# ---------------------------------------------------------------------------
# Generate mode
# ---------------------------------------------------------------------------

echo "Generating CycloneDX SBOM for nexus-os v${PROJECT_VERSION}..."

TMPDIR_SBOM=$(mktemp -d)
trap 'rm -rf "$TMPDIR_SBOM"' EXIT

RUST_FILE="$TMPDIR_SBOM/rust.json"
NPM_FILE="$TMPDIR_SBOM/npm.json"

parse_cargo_lock > "$RUST_FILE"
parse_npm_lock > "$NPM_FILE"

RUST_COUNT=$(python3 -c "import json; print(len(json.load(open('$RUST_FILE'))))" 2>/dev/null || echo "?")
NPM_COUNT=$(python3 -c "import json; print(len(json.load(open('$NPM_FILE'))))" 2>/dev/null || echo "?")

# Merge into CycloneDX document
python3 - "$RUST_FILE" "$NPM_FILE" "$OUTPUT" "$SERIAL" "$TIMESTAMP" "$PROJECT_VERSION" <<'PYEOF'
import json, sys

rust_file, npm_file, output, serial, timestamp, version = sys.argv[1:7]

with open(rust_file) as f:
    rust = json.load(f)
with open(npm_file) as f:
    npm = json.load(f)

sbom = {
    "bomFormat": "CycloneDX",
    "specVersion": "1.5",
    "version": 1,
    "serialNumber": serial,
    "metadata": {
        "timestamp": timestamp,
        "tools": [{
            "vendor": "nexus-os",
            "name": "nexus-sbom",
            "version": version
        }],
        "component": {
            "type": "application",
            "name": "nexus-os",
            "version": version
        },
        "authors": [{"name": "Suresh Karicheti"}]
    },
    "components": rust + npm
}

with open(output, "w") as f:
    json.dump(sbom, f, indent=2)
    f.write("\n")
PYEOF

echo "SBOM written to: $OUTPUT"
echo "  Rust crates:    $RUST_COUNT"
echo "  npm packages:   $NPM_COUNT"
echo "  Total:          $(python3 -c "print($RUST_COUNT + $NPM_COUNT)" 2>/dev/null || echo '?')"
echo "Done."
