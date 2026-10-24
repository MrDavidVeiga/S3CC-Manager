# Build a real PR #2 Windows x64 portable EXE on the user's PC.
# GitHub Actions and its billing status are not involved. No test gate.
param([switch]$RunTests)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$out = Join-Path $root "TESTE-PR2-BETA3-WINDOWS-X64"
$log = Join-Path $out "build.log"
$oldDir = Get-Location
$target = "x86_64-pc-windows-msvc"
function Step {
    param([string]$Name, [string]$Program, [string[]]$ArgsList)
    Write-Host ("[PR2] " + $Name) -ForegroundColor Cyan
    Add-Content -LiteralPath $log -Value ("==== " + $Name + " ====")
    & $Program @ArgsList 2>&1 | Tee-Object -FilePath $log -Append | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw ("Failed: " + $Name + " (exit " + $LASTEXITCODE + "). Log: " + $log)
    }
}
try {
    Set-Location $root
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    "PR2 Windows x64 test build log" | Set-Content -LiteralPath $log
    foreach ($tool in @("git.exe", "node.exe", "npm.cmd", "cargo.exe", "rustup.exe")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw ("Missing " + $tool + ". Install Git, Node.js LTS and Rust MSVC tools first.")
        }
    }
    if (-not [Environment]::Is64BitOperatingSystem) { throw "Windows x64 required." }
    $branch = (& git branch --show-current).Trim()
    if ($LASTEXITCODE -ne 0 -or $branch -ne "fix/beta3-elif-stability-20261024") {
        throw ("Wrong branch: " + $branch + ". Select fix/beta3-elif-stability-20261024.")
    }
    $commit = (& git rev-parse --short=12 HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw "Cannot determine Git commit." }
    Step "Install npm dependencies" "npm.cmd" @("install")
    $b64Path = Join-Path $root "src-tauri/icons/manager-icon.b64"
    $pngPath = Join-Path $root "src-tauri/icons/manager-icon.png"
    $raw = (Get-Content -LiteralPath $b64Path -Raw).Trim()
    [IO.File]::WriteAllBytes($pngPath, [Convert]::FromBase64String($raw))
    Step "Generate Tauri application icons" "npm.cmd" @("run","tauri","icon","src-tauri/icons/manager-icon.png")
    Step "Install Windows Rust target" "rustup.exe" @("target","add",$target)
    Step "Compile portable Windows executable" "npm.cmd" @("run","tauri","build","--","--target",$target,"--no-bundle")
    $exe = Join-Path $root ("src-tauri/target/" + $target + "/release/s3cc-organizer.exe")
    if (-not (Test-Path -LiteralPath $exe)) {
        $exe = Join-Path $root "src-tauri/target/release/s3cc-organizer.exe"
    }
    if (-not (Test-Path -LiteralPath $exe)) { throw "Compiler finished but EXE was not found." }
    if ((Get-Item -LiteralPath $exe).Length -lt 100000) { throw "EXE is too small; build not trusted." }
    $stage = Join-Path $out "portable"
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    $name = "Veiga's S3CC Manager - Beta 3 PR2 Test.exe"
    $exeDest = Join-Path $stage $name
    Copy-Item -LiteralPath $exe -Destination $exeDest -Force
    $readme = @"
PR2 BETA 3 WINDOWS X64 — TEST BUILD
Source branch: fix/beta3-elif-stability-20261024
Commit: $commit
Not an official release. No GitHub Actions used.
Use a COPY of your Mods folder.
Check Duplicates scrollbar dragging, expandable Duplicates/Conflicts,
and Health analysis progress plus Cancel.
Share steps and build.log if a problem happens.
"@
    $readme | Set-Content -LiteralPath (Join-Path $stage "LEIA-ME-TESTE.txt") -Encoding UTF8
    $zip = Join-Path $out ("VeigasS3CCManager_PR2_Beta3_Test_Windows_x64_" + $commit + ".zip")
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -Force
    $sha = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
    ($sha + "  " + (Split-Path $zip -Leaf)) | Set-Content -LiteralPath (Join-Path $out "SHA256.txt") -Encoding Ascii
    Write-Host ("[PR2] EXE: " + $exeDest) -ForegroundColor Green
    Write-Host ("[PR2] ZIP: " + $zip) -ForegroundColor Green
    Write-Host ("[PR2] BUILD LOG: " + $log)
    if ($RunTests) {
        try {
            Step "JavaScript syntax" "node.exe" @("--check","src/app.js")
            foreach ($file in @("test-beta3-elif-stability.mjs","test-expandable-analysis-ui.mjs",
                "test-health-diagnostics.mjs","test-analysis-action-flow.mjs",
                "test-final-ui-selection.mjs","test-loading-overlay-state.mjs",
                "test-performance-architecture.mjs")) {
                Step ("Regression " + $file) "node.exe" @("scripts/" + $file)
            }
            Step "Rust library tests" "cargo.exe" @("test","--manifest-path","src-tauri/Cargo.toml","--lib")
        } catch {
            Write-Warning ("A test failed, but your compiled EXE remains available. " + $_.Exception.Message)
        }
    }
    Start-Process "explorer.exe" -ArgumentList ('"' + $out + '"')
} catch {
    Write-Host ("[PR2] BUILD ERROR: " + $_.Exception.Message) -ForegroundColor Red
    Write-Host ("[PR2] LOG: " + $log)
    exit 1
} finally {
    Set-Location $oldDir
}
