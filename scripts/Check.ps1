$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $root
try {
    $env:CARGO_TARGET_DIR = Join-Path $root 'target'
    cargo fmt --all --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
    cargo clippy --locked --offline --all-targets -j 2 -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy failed.' }
    cargo test --locked --offline -j 2
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    node --test extension/tests/*.test.js
    if ($LASTEXITCODE -ne 0) { throw 'Extension tests failed.' }
    Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.ps1' | ForEach-Object {
        $parseErrors = $null
        [System.Management.Automation.Language.Parser]::ParseFile($_.FullName, [ref]$null, [ref]$parseErrors) | Out-Null
        if ($parseErrors.Count) { throw "$($_.Name): $parseErrors" }
    }
} finally { Pop-Location }
