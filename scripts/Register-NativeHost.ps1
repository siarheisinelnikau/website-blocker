[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][ValidatePattern('^[a-p]{32}$')][string]$ExtensionId,
    [ValidateSet('Chrome', 'Edge')][string]$Browser = 'Chrome'
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Native registration currently supports Windows only.' }
$resolvedExe = (Resolve-Path -LiteralPath $ExePath).Path
if ([IO.Path]::GetExtension($resolvedExe) -ne '.exe') { throw 'ExePath must be an executable.' }
$hostName = 'com.websiteblocker.companion'
$vendor = if ($Browser -eq 'Chrome') { 'Google\Chrome' } else { 'Microsoft\Edge' }
$key = "HKCU:\Software\$vendor\NativeMessagingHosts\$hostName"
$directory = Join-Path $env:LOCALAPPDATA 'WebsiteBlocker\native'
$manifestPath = Join-Path $directory "$Browser.json"
$manifest = @{ name = $hostName; description = 'WebsiteBlocker local rule companion'; path = $resolvedExe; type = 'stdio'; allowed_origins = @("chrome-extension://$ExtensionId/") }
if ($PSCmdlet.ShouldProcess("$key -> $manifestPath", 'Register per-user WebsiteBlocker native host')) {
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
    New-Item -Path $key -Force | Out-Null
    Set-Item -LiteralPath $key -Value $manifestPath
    Write-Output "Registered $Browser for extension $ExtensionId. Click Sync with desktop in the extension popup."
}
