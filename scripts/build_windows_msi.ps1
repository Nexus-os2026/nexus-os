$ErrorActionPreference = "Stop"

param(
    [string]$Version = "1.0.0"
)

Write-Host "[windows] building nexus-cli release binary"
cargo build --release -p nexus-cli

if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
    Write-Warning "WiX Toolset (wix) not found. MSI build step skipped; manifest is generated."
    Exit 0
}

Write-Host "[windows] building MSI from WiX manifest"
$outDir = "target/package/windows"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

wix build `
  packaging/windows/nexus-os.wxs `
  -o "$outDir/nexus-os-$Version.msi"

Write-Host "[windows] output: $outDir/nexus-os-$Version.msi"
