param(
    [ValidateSet('Chrome','Edge')][string]$Browser,
    [ValidateSet('Register','Backup','Restore')][string]$Mode,
    [string]$Manifest,
    [string]$Backup
)
$ErrorActionPreference = 'Stop'
$vendor = if ($Browser -eq 'Chrome') { 'Google\Chrome' } else { 'Microsoft\Edge' }
$key = "Software\$vendor\NativeMessagingHosts\com.websiteblocker.companion"
if ($Mode -ne 'Restore') {
    if (Test-Path -LiteralPath $Backup) { throw 'A prior fixture backup exists. Restore it before another test.' }
    $existing = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($key)
    $snapshot = @{ exists = ($null -ne $existing); hasDefault = $false; value = $null; kind = 'String' }
    if ($existing) {
        $snapshot.hasDefault = $existing.GetValueNames() -contains ''
        if ($snapshot.hasDefault) {
            $snapshot.value = $existing.GetValue('', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $snapshot.kind = $existing.GetValueKind('').ToString()
        }
        $existing.Dispose()
    }
    [IO.File]::WriteAllText($Backup, ($snapshot | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    if ($Mode -eq 'Backup') { return }
    $opened = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($key)
    try { $opened.SetValue('', $Manifest, [Microsoft.Win32.RegistryValueKind]::String) } finally { $opened.Dispose() }
} else {
    if (-not (Test-Path -LiteralPath $Backup)) { return }
    $snapshot = Get-Content -LiteralPath $Backup -Raw | ConvertFrom-Json
    $opened = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($key, $true)
    if ($opened) {
        try {
            if ($opened.GetValue('') -ne $Manifest) {
                if ($snapshot.hasDefault -and $opened.GetValue('') -eq $snapshot.value) { Remove-Item -LiteralPath $Backup; return }
                throw 'Registration changed outside the test. Refusing to overwrite it.'
            }
            if ($snapshot.hasDefault) {
                $opened.SetValue('', $snapshot.value, [Enum]::Parse([Microsoft.Win32.RegistryValueKind], $snapshot.kind))
            } else { $opened.DeleteValue('', $false) }
        } finally { $opened.Dispose() }
    }
    if (-not $snapshot.exists) { [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKey($key, $false) }
    Remove-Item -LiteralPath $Backup
}
