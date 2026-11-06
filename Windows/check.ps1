# The same Rust engine checks run on all desktop platforms.
param([ValidateSet("check", "update")][string]$Mode = "check")
$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")
try {
    cargo xtask check $Mode
    if ($LASTEXITCODE -ne 0) { throw "Rust engine checks failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
}
