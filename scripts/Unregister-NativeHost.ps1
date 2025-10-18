[CmdletBinding(SupportsShouldProcess)]
param([ValidateSet('Chrome', 'Edge')][string]$Browser = 'Chrome')
$ErrorActionPreference = 'Stop'
$vendor = if ($Browser -eq 'Chrome') { 'Google\Chrome' } else { 'Microsoft\Edge' }
$key = "HKCU:\Software\$vendor\NativeMessagingHosts\com.websiteblocker.companion"
$manifestPath = Join-Path $env:LOCALAPPDATA "WebsiteBlocker\native\$Browser.json"
if (Test-Path -LiteralPath $key) {
    $current = (Get-Item -LiteralPath $key).GetValue('')
    if ($current -ne $manifestPath) { throw 'Registration points to another manifest. Nothing was changed.' }
    if ($PSCmdlet.ShouldProcess($key, 'Remove WebsiteBlocker native host registration')) {
        Remove-Item -LiteralPath $key
        if (Test-Path -LiteralPath $manifestPath) { Remove-Item -LiteralPath $manifestPath }
    }
}
Write-Output 'Local settings and the browser extension are preserved. Remove the extension in the browser separately if desired.'
