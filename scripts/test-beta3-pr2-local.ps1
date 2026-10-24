# Windows-only validation for PR #2. No CC files are modified.
# Run from any location:
#   powershell -ExecutionPolicy Bypass -File .\scripts\test-beta3-pr2-local.ps1
param(
    [switch]$SkipFrontendBuild,
    [switch]$SkipRust
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$log = Join-Path ([IO.Path]::GetTempPath()) ("S3CC-PR2-test-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".log")
$oldDirectory = Get-Location

function Run-Step {
    param([string]$Label, [string]$Command, [string[]]$Arguments)
    Write-Host ("[PR2] " + $Label) -ForegroundColor Cyan
    ("=== " + $Label + " ===") | Out-File -FilePath $log -Append -Encoding UTF8
    & $Command @Arguments 2>&1 | Tee-Object -FilePath $log -Append
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE. Full output: $log"
    }
}

try {
    Set-Location $root
    if (!(Get-Command "node" -ErrorAction SilentlyContinue)) {
        throw "Node.js was not found. Install Node.js LTS and retry."
    }
    if (!(Get-Command "npm.cmd" -ErrorAction SilentlyContinue)) {
        throw "npm.cmd was not found. Install Node.js LTS and retry."
    }
    Write-Host "[PR2] Validation started. Log: $log"
    Run-Step "JavaScript syntax" "node" @("--check", "src/app.js")
    foreach ($test in @(
        "test-beta3-elif-stability.mjs",
        "test-expandable-analysis-ui.mjs",
        "test-health-diagnostics.mjs",
        "test-analysis-action-flow.mjs",
        "test-final-ui-selection.mjs",
        "test-loading-overlay-state.mjs",
        "test-performance-architecture.mjs"
    )) {
        Run-Step "Regression $test" "node" @("scripts/$test")
    }
    if (!$SkipFrontendBuild) {
        if (!(Test-Path (Join-Path $root "node_modules"))) {
            Run-Step "Install frontend dependencies" "npm.cmd" @("install")
        }
        Run-Step "Vite production build" "npm.cmd" @("run", "build")
    }
    if (!$SkipRust) {
        if (!(Get-Command "cargo" -ErrorAction SilentlyContinue)) {
            throw "Rust cargo not found. Install the Rust stable toolchain or rerun with -SkipRust."
        }
        $iconPath = Join-Path $root "src-tauri/icons/manager-icon.png"
        if (!(Test-Path $iconPath)) {
            $raw = (Get-Content (Join-Path $root "src-tauri/icons/manager-icon.b64") -Raw).Trim()
            [IO.File]::WriteAllBytes($iconPath, [Convert]::FromBase64String($raw))
        }
        if (!(Test-Path (Join-Path $root "src-tauri/icons/icon.ico"))) {
            if (!(Test-Path (Join-Path $root "node_modules"))) {
                Run-Step "Install frontend dependencies for Tauri" "npm.cmd" @("install")
            }
            Run-Step "Generate required Tauri icons" "npm.cmd" @("run", "tauri", "icon", "src-tauri/icons/manager-icon.png")
        }
        Run-Step "Rust library tests" "cargo" @("test", "--manifest-path", "src-tauri/Cargo.toml", "--lib")
    }
    Write-Host "[PR2] Requested automated checks passed." -ForegroundColor Green
    Write-Host "[PR2] Log: $log"
} catch {
    Write-Host "[PR2] FAILED: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "[PR2] Log: $log"
    exit 1
} finally {
    Set-Location $oldDirectory
}
