param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location -LiteralPath $root
try {
    $env:CARGO_TARGET_DIR = Join-Path $root 'target'
    $cargoArgs = @('build', '--release', '--locked', '--target', 'x86_64-pc-windows-msvc', '-j', '2')
    if ($Offline) { $cargoArgs += '--offline' }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $metadataText = & cargo metadata --no-deps --format-version 1 --offline
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $version = ($metadataText | ConvertFrom-Json).packages[0].version
    $manifest = Get-Content -LiteralPath 'extension/manifest.json' -Raw | ConvertFrom-Json
    if ($manifest.version -ne $version) { throw 'Extension and desktop versions differ.' }
    New-Item -ItemType Directory -Path 'dist' -Force | Out-Null
    $exeName = "WebsiteBlocker-v$version.exe"
    Copy-Item -LiteralPath 'target/x86_64-pc-windows-msvc/release/website-blocker.exe' -Destination "dist/$exeName"
    $companionName = "WebsiteBlocker-Companion-v$version.zip"
    Compress-Archive -Path 'extension', 'scripts', 'docs', 'README.md', 'LICENSE' -DestinationPath "dist/$companionName" -Force
    $lines = @($exeName, $companionName) | ForEach-Object {
        $hash = (Get-FileHash -LiteralPath "dist/$_" -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $_"
    }
    [IO.File]::WriteAllLines((Join-Path $root 'dist/checksums.txt'), $lines, [Text.UTF8Encoding]::new($false))
    Write-Output 'Local artifacts prepared in dist/. Nothing was uploaded or published.'
} finally { Pop-Location }

