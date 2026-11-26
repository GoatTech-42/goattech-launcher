param(
 [Parameter(Mandatory=$true)][string]$Binary,
 [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedSha256,
 [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{40}$')][string]$SourceCommit,
 [Parameter(Mandatory=$true)][string]$Out,
 [Parameter(Mandatory=$true)][string]$SourceRoot,
 [switch]$IsolatedWindowsSession
)
$ErrorActionPreference='Stop'
# Explicit acknowledgement, not automatic authority to touch an existing desktop.
if (-not $IsolatedWindowsSession -or $env:OS -ne 'Windows_NT') {throw 'Requires an approved isolated Windows session'}
if ($env:GITHUB_ACTIONS -eq 'true' -and $env:GITHUB_REPOSITORY_VISIBILITY -ne 'private') {throw 'Only a private repository may run this capture'}
if (Test-Path -LiteralPath $Out) {throw 'Output must be a new private local directory'}
$outPath=[IO.Path]::GetFullPath($Out)
$sourcePath=(Resolve-Path -LiteralPath $SourceRoot).Path.TrimEnd('\','/')
if ($outPath.Equals($sourcePath,[StringComparison]::OrdinalIgnoreCase) -or $outPath.StartsWith($sourcePath+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'Output cannot be inside source checkout'}
$ancestor=Split-Path -Parent $outPath
while (-not (Test-Path -LiteralPath $ancestor)) {$parent=Split-Path -Parent $ancestor;if ($parent -eq $ancestor -or -not $parent) {throw 'No existing output ancestor'};$ancestor=$parent}
# Windows PowerShell5.1 can promote expected native stderr to a terminating error.
# Keep that expected non-repository result local; restore strict errors immediately.
$previousErrorAction=$ErrorActionPreference
try {
 $ErrorActionPreference='Continue'
 $worktree=@(& git -C $ancestor rev-parse --is-inside-work-tree 2>&1)
 $gitExit=$LASTEXITCODE
} finally {$ErrorActionPreference=$previousErrorAction}
if ($gitExit -eq 0 -and $worktree -contains 'true') {throw 'Output cannot be inside any git work tree'}
if ($gitExit -ne 0 -and $gitExit -ne 128) {throw 'Git output-path check failed unexpectedly'}
$Out=$outPath
$binaryPath=(Resolve-Path -LiteralPath $Binary).Path
$actual=(Get-FileHash -LiteralPath $binaryPath -Algorithm SHA256).Hash
if ($actual -ne $ExpectedSha256) {throw 'Binary hash mismatch'}
if ((& git -C $SourceRoot rev-parse HEAD).Trim() -ne $SourceCommit) {throw 'Source checkout mismatch'}
$consentSource=Get-Content -LiteralPath (Join-Path $SourceRoot 'crates/horizon-telemetry/src/lib.rs') -Raw
$schema=[regex]::Match($consentSource,'pub const CONSENT_VERSION: u32 = ([0-9]+);')
if (-not $schema.Success) {throw 'Consent schema unknown'}
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class PrivateStageCapture {
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint f);
 public struct Rect { public int L,T,R,B; }
}
'@
$fixtureRoot=Join-Path $Out 'owned-fixture'
$names=@('HORIZON_LAUNCHER_HOME','HORIZON_HOME','HORIZON_LAUNCHER_DESKTOP','HORIZON_TELEMETRY_URL','HORIZON_LAUNCHER_NO_SELF_UPDATE')
$saved=@{}
foreach ($name in $names) {$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$p=$null;$frames=[Collections.Generic.List[object]]::new();$complete=$false;$closed=$false
$clock=[Diagnostics.Stopwatch]::StartNew()
try {
 New-Item -ItemType Directory -Path $Out,$fixtureRoot | Out-Null
 $env:HORIZON_LAUNCHER_HOME=Join-Path $fixtureRoot 'launcher'
 $env:HORIZON_HOME=Join-Path $fixtureRoot 'game'
 $env:HORIZON_LAUNCHER_DESKTOP=Join-Path $fixtureRoot 'desktop'
 $env:HORIZON_TELEMETRY_URL='off'
 $env:HORIZON_LAUNCHER_NO_SELF_UPDATE='1'
 New-Item -ItemType Directory -Path $env:HORIZON_LAUNCHER_HOME,(Join-Path $env:HORIZON_HOME 'telemetry') | Out-Null
 @{install_new_versions=$false;skip_opening=$true;owner_notice_seen=$true;ping_notice_seen=$true;basic_summaries=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $env:HORIZON_LAUNCHER_HOME 'settings.json')
 @{version=[int]$schema.Groups[1].Value;answered=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds();choices=@{performance=$false;crashes=$false;bugs=$false;benchmarks=$false}} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $env:HORIZON_HOME 'telemetry/consent.json')

 $p=Start-Process -FilePath $binaryPath -PassThru -RedirectStandardOutput (Join-Path $Out 'private.stdout') -RedirectStandardError (Join-Path $Out 'private.stderr')
 # Capture earliest available client pixels and then8seconds of passive stage.
 # No click/sign-in/game/installer/download command is issued by this harness.
 $deadline=[DateTime]::UtcNow.AddSeconds(60)
 do {$p.Refresh();if ($p.HasExited) {throw 'App exited before window'};if ($p.MainWindowHandle -ne 0) {break};Start-Sleep -Milliseconds 50} while ([DateTime]::UtcNow -lt $deadline)
 if ($p.MainWindowHandle -eq 0) {throw 'No owned window within60seconds'}
 $start=$clock.Elapsed.TotalMilliseconds
 foreach ($offset in @(0,250,500,1000,2000,4000,6000,8000)) {
  while ($clock.Elapsed.TotalMilliseconds -lt $start+$offset) {Start-Sleep -Milliseconds 10}
  $p.Refresh();if ($p.HasExited) {throw 'App exited during capture'}
  $h=$p.MainWindowHandle;[uint32]$owner=0
  [void][PrivateStageCapture]::GetWindowThreadProcessId($h,[ref]$owner)
  if ($owner -ne $p.Id -or -not [PrivateStageCapture]::IsWindowVisible($h)) {throw 'Window ownership/visibility changed'}
  $r=New-Object PrivateStageCapture+Rect
  if (-not [PrivateStageCapture]::GetClientRect($h,[ref]$r)) {throw 'No client geometry'}
  $w=$r.R-$r.L;$height=$r.B-$r.T
  if ($w -lt 200 -or $height -lt 200 -or $w -gt 3840 -or $height -gt 2160) {throw 'Client size outside bounded capture'}
  $file=('frame-{0:D5}.png' -f $offset);$path=Join-Path $Out $file
  $bitmap=New-Object Drawing.Bitmap($w,$height)
  try {
   $g=[Drawing.Graphics]::FromImage($bitmap)
   try {$dc=$g.GetHdc();try {if (-not [PrivateStageCapture]::PrintWindow($h,$dc,3)) {throw 'Capture API refused'}} finally {$g.ReleaseHdc($dc)}} finally {$g.Dispose()}
   $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Png)
  } finally {$bitmap.Dispose()}
  $frames.Add(@{file=$file;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash;width=$w;height=$height;requested_offset_ms=$offset;observed_ms=$clock.Elapsed.TotalMilliseconds})
 }
 $complete=$true
} finally {
 if ($null -eq $p) {$closed=$true}
 if ($null -ne $p) {
  $p.Refresh()
  if (-not $p.HasExited) {[void]$p.CloseMainWindow();$closed=$p.WaitForExit(15000)} else {$closed=$true}
  $p.Dispose()
 }
 foreach ($name in $names) {[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}
 # Keep failed fixture state for diagnosis; never delete while its process may live.
 if ($closed -and (Test-Path -LiteralPath $fixtureRoot)) {Remove-Item -LiteralPath $fixtureRoot -Recurse}
 $receipt=@{capture_complete=$complete;process_closed=$closed;binary_sha256=$actual;source_commit_claim=$SourceCommit;source_binary_binding='caller supplied, not independently established';frames=@($frames);visual_verdict='UNVERIFIED until private pixel review';limits=@('startup may finish before first frame','synthetic all-off consent fixture; page identity requires pixel review','no timing/performance or animation smoothness verdict','app may make its own release/link requests; no auth or game commands','no install performed; no exporter or uploader','private.stdout and private.stderr retained locally; may contain app diagnostics','DPI awareness inherited from caller; not changed or measured')}
 $receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Out 'capture-receipt.json')
 if (-not $closed) {Write-Warning 'Owned app did not close; fixture retained, manual exact-process cleanup required'}
}
