#requires -Version 5.1
<#
  Offline-friendly Windows verification entry point for Neramit Planner.
  Run from PowerShell after installing the official Rust stable toolchain:
    powershell -ExecutionPolicy Bypass -File .\scripts\verify-project-controls.ps1
  This script never uploads project files and never publishes a release.
#>
[CmdletBinding()]
param([switch]$SkipSmoke)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$outDir = Join-Path $root "test-results"
New-Item -Path $outDir -ItemType Directory -Force | Out-Null

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Rust Cargo is not installed. Install the official Rust stable toolchain first."
}

function Invoke-CargoCheck {
    param([string]$Label, [string[]]$Arguments)
    $log = Join-Path $outDir ("{0}.log" -f $Label)
    Write-Host ("Starting {0} (log: {1})" -f $Label, $log)
    # PowerShell 5.1 can treat native stderr as a terminating error under Stop.
    # Capture it as text, then rely strictly on Cargo's exit code.
    $previousAction = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & cargo @Arguments 2>&1 | Tee-Object -FilePath $log
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousAction
    }
    if ($code -ne 0) {
        throw ("{0} failed with exit code {1}. Inspect {2}" -f $Label, $code, $log)
    }
    Write-Host ("PASS {0}" -f $Label)
}

Push-Location $root
try {
    & cargo --version
    & rustc --version
    Invoke-CargoCheck -Label "cargo-test" -Arguments @("test", "--locked", "--verbose")
    Invoke-CargoCheck -Label "cargo-build" -Arguments @("build", "--locked")

    if (-not $SkipSmoke) {
        $exe = Join-Path $root "target\debug\NeramitPlanner.exe"
        $sample = Join-Path $root "examples\sample.neramit"
        if (-not (Test-Path $exe)) {
            throw ("The built executable is missing: {0}" -f $exe)
        }
        if (-not (Test-Path $sample)) {
            throw ("The smoke-test fixture is missing: {0}" -f $sample)
        }
        $sampleOut = Join-Path $outDir "sample-roundtrip.neramit"
        if (Test-Path $sampleOut) {
            Remove-Item $sampleOut -Force
        }
        & $exe --convert $sample $sampleOut
        if (($LASTEXITCODE -ne 0) -or (-not (Test-Path $sampleOut))) {
            throw "Native sample conversion smoke test failed."
        }
        Write-Host "PASS sample .neramit conversion smoke test"
    }
    Write-Host ("Verification commands completed successfully. Logs: {0}" -f $outDir)
} finally {
    Pop-Location
}
