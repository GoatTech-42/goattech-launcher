param([Parameter(Mandatory=$true)][string]$Candidate,[Parameter(Mandatory=$true)][string]$Out)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {throw 'Hosted Windows runner only'}
$root=Join-Path $env:RUNNER_TEMP 'goattech-package-qualification'
if (Test-Path $root) {throw 'Qualification root already exists'}
if (Test-Path 'HKCU:\Software\GoatTech\Launcher') {throw 'Existing GoatTech install, refusing fixture writes'}
New-Item -ItemType Directory $root,$Out | Out-Null
$env:HORIZON_LAUNCHER_HOME=Join-Path $root 'profile'
$env:HORIZON_HOME=Join-Path $root 'game'
$env:HORIZON_LAUNCHER_DESKTOP=Join-Path $root 'desktop'
$env:HORIZON_TELEMETRY_URL='off'
$env:HORIZON_LAUNCHER_NO_SELF_UPDATE='1'
New-Item -ItemType Directory $env:HORIZON_LAUNCHER_HOME,$env:HORIZON_HOME,$env:HORIZON_LAUNCHER_DESKTOP | Out-Null
$checks=[Collections.Generic.List[string]]::new()
function Check([bool]$ok,[string]$name) {if (-not $ok) {throw $name};$checks.Add($name)}
function Hash([string]$p) {(Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash}
function Run([string]$exe,[string[]]$arguments) {
 $p=Start-Process -FilePath $exe -ArgumentList $arguments -PassThru -Wait
 try {Check ($p.ExitCode -eq 0) ('command-'+($arguments -join '-'))} finally {$p.Dispose()}
}
function Open-Capture-Close([string]$exe,[string]$name) {
 $p=Start-Process -FilePath $exe -PassThru -RedirectStandardOutput (Join-Path $Out ($name+'.stdout')) -RedirectStandardError (Join-Path $Out ($name+'.stderr'))
 try {
  $deadline=[DateTime]::UtcNow.AddSeconds(60)
  do {$p.Refresh();if ($p.HasExited) {throw 'App exited before visible window'};if ($p.MainWindowHandle -ne 0) {break};Start-Sleep -Milliseconds 250} while ([DateTime]::UtcNow -lt $deadline)
  Check ($p.MainWindowHandle -ne 0) ($name+'-owned-window')
  # Capture child has a finite outer workflow timeout. Pixels still need review.
  & pwsh -NoProfile -File tools/qa/windows-native-capture.ps1 -OwnedPid $p.Id -Out (Join-Path $Out ($name+'.png'))
  Check ($LASTEXITCODE -eq 0) ($name+'-capture')
  Check ($p.CloseMainWindow()) ($name+'-close-request')
  Check ($p.WaitForExit(20000)) ($name+'-clean-close')
  Check ($p.ExitCode -eq 0) ($name+'-zero-exit')
 } finally {
  $p.Refresh()
  if (-not $p.HasExited) {[void]$p.CloseMainWindow();[void]$p.WaitForExit(5000)}
  $p.Dispose()
 }
}
$zip=@(Get-ChildItem -LiteralPath $Candidate -Filter '*windows-x64.zip')
Check ($zip.Count -eq 1) 'one-windows-archive'
$extract=Join-Path $root 'extracted'
Expand-Archive -LiteralPath $zip[0].FullName -DestinationPath $extract
$exe=Join-Path $extract 'horizon-launcher.exe'
$setup=Join-Path $Candidate 'horizon-launcher-setup.exe'
$inputHash=Hash $exe
Check ((Hash $setup) -eq $inputHash) 'setup-equals-extracted-binary'
Open-Capture-Close $exe 'portable'
$profileSentinel=Join-Path $env:HORIZON_LAUNCHER_HOME 'synthetic-preserve.txt'
$gameSentinel=Join-Path $env:HORIZON_HOME 'synthetic-world.txt'
[IO.File]::WriteAllText($profileSentinel,'owned synthetic profile sentinel')
[IO.File]::WriteAllText($gameSentinel,'owned synthetic game sentinel')
$profileHash=Hash $profileSentinel;$gameHash=Hash $gameSentinel
$installBase=Join-Path $root 'install'
$installed=Join-Path $installBase 'GoatTech Launcher\horizon-launcher.exe'
$installedOnce=$false
$complete=$false
try {
 Run $setup @('setup','--quiet','--dir',('"'+$installBase+'"'))
 $installedOnce=$true
 Check ((Hash $installed) -eq $inputHash) 'installed-equals-package-binary'
 $unowned=Join-Path (Split-Path $installed) 'synthetic-unowned.txt'
 [IO.File]::WriteAllText($unowned,'do not delete on reinstall or uninstall')
 $unownedHash=Hash $unowned
 # Same-version reinstall tests replacement/preservation, not remote auto-update.
 Run $setup @('setup','--quiet','--dir',('"'+$installBase+'"'))
 Check ((Hash $installed) -eq $inputHash) 'reinstalled-equals-package-binary'
 Check ((Hash $unowned) -eq $unownedHash) 'reinstall-preserves-unowned-file'
 Check ((Hash $profileSentinel) -eq $profileHash) 'reinstall-preserves-profile'
 Check ((Hash $gameSentinel) -eq $gameHash) 'reinstall-preserves-game'
 Open-Capture-Close $installed 'installed'
 Run $installed @('uninstall','--quiet')
 $installedOnce=$false
 Check (-not (Test-Path $installed)) 'uninstall-removes-owned-binary'
 Check ((Hash $unowned) -eq $unownedHash) 'uninstall-preserves-unowned-file'
 Check ((Hash $profileSentinel) -eq $profileHash) 'uninstall-preserves-profile'
 Check ((Hash $gameSentinel) -eq $gameHash) 'uninstall-preserves-game'
 Check (-not (Test-Path 'HKCU:\Software\GoatTech\Launcher')) 'uninstall-removes-registration'
 $complete=$true
} finally {
 if ($installedOnce -and (Test-Path $installed)) {Run $installed @('uninstall','--quiet')}
 $receipt=@{complete=$complete;source=(& git rev-parse HEAD);binary_sha256=$inputHash;checks=@($checks);scope='no-auth portable/install/reinstall/uninstall; pixels awaiting review; not gameplay or remote updater'}
 $receipt | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Out 'package-receipt.json')
}
