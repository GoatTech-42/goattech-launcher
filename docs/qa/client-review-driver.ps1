$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $env:CAP | Out-Null
$info = "$env:CAP\window-info.txt"
$src = 'using System;using System.Runtime.InteropServices;public class WinCap{[DllImport("user32.dll",SetLastError=true)]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);[DllImport("user32.dll")]public static extern IntPtr GetLastActivePopup(IntPtr h);[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]public static extern uint GetDpiForWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool GetClientRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);[DllImport("user32.dll")]public static extern bool IsIconic(IntPtr h);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h2,bool r);[DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);[DllImport("user32.dll")]public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool GetCursorPos(out POINT p);[DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(POINT p);[DllImport("user32.dll")]public static extern IntPtr GetAncestor(IntPtr h,uint flags);public struct POINT{public int X,Y;}public struct RECT{public int Left,Top,Right,Bottom;}}'
if (-not ('WinCap' -as [type])) { Add-Type -TypeDefinition $src -ReferencedAssemblies System.Drawing }
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$bin = "$PWD\launcher\target\release\horizon-launcher.exe"
$actual = (git rev-parse HEAD).Trim()
if ($actual -ne 'cd5d9ce4b7c2e4f0e9ce0c4e28490c0fc91b6e1e') { throw 'Unexpected instrumented source' }
"source-commit: $actual; shipping-source: 235b8a9b7d920e36704b508b4136808976e91287; instrumented fixture, NOT exact shipping-tip evidence" | Out-File $info
"workflow-commit: $env:GITHUB_SHA" | Out-File $info -Append
"rustc: $env:CAP_RUSTC_VERSION" | Out-File $info -Append
"cargo: $env:CAP_CARGO_VERSION" | Out-File $info -Append
"binary-sha256: $((Get-FileHash $bin -Algorithm SHA256).Hash)" | Out-File $info -Append
'network: telemetry off and no-self-update requested; no zero-network claim; builds use network' | Out-File $info -Append
$root = Join-Path $env:RUNNER_TEMP "client-review-$env:CAP_ROUTE-$env:CAP_WIDTH"
if (Test-Path $root) { throw 'Expected fresh isolated root' }
New-Item -ItemType Directory $root | Out-Null
$env:HORIZON_LAUNCHER_HOME = Join-Path $root 'launcher'
$env:HORIZON_HOME = Join-Path $root 'game'
$env:HOME = Join-Path $root 'home'; $env:USERPROFILE = $env:HOME
$env:APPDATA = Join-Path $root 'appdata'; $env:LOCALAPPDATA = Join-Path $root 'localappdata'
$env:XDG_DATA_HOME = Join-Path $root 'xdg-data'
$env:XDG_CONFIG_HOME = Join-Path $root 'xdg-config'
$env:HORIZON_LAUNCHER_DESKTOP = Join-Path $root 'desktop'
foreach ($path in @($env:HORIZON_LAUNCHER_HOME,$env:HORIZON_HOME,$env:HOME,$env:APPDATA,$env:LOCALAPPDATA,$env:XDG_DATA_HOME,$env:XDG_CONFIG_HOME,$env:HORIZON_LAUNCHER_DESKTOP)) { New-Item -ItemType Directory -Force $path | Out-Null }
@{install_new_versions=$false;skip_opening=$true;owner_notice_seen=$true;ping_notice_seen=$true;basic_summaries=$false} | ConvertTo-Json | Set-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'settings.json')
# Test profile only: record the same all-off consent choices as Keep optional data off.
# Telemetry endpoint=off does NOT answer the first-run question. Derive the schema
# version from the exact pinned source and fail closed if that contract changes.
$consentSource=Get-Content -LiteralPath (Join-Path $PWD 'crates/horizon-telemetry/src/lib.rs') -Raw
$versionMatch=[regex]::Match($consentSource,'pub const CONSENT_VERSION: u32 = ([0-9]+);')
if (-not $versionMatch.Success) { throw 'Cannot establish consent schema version' }
$consentVersion=[int]$versionMatch.Groups[1].Value
$consentDir=Join-Path $env:HORIZON_HOME 'telemetry'
New-Item -ItemType Directory -Force $consentDir | Out-Null
$consentPath=Join-Path $consentDir 'consent.json'
if (Test-Path -LiteralPath $consentPath) { throw 'Expected unanswered fresh test profile' }
@{version=$consentVersion;answered=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds();choices=@{performance=$false;crashes=$false;bugs=$false;benchmarks=$false}} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $consentPath
$consentHash=(Get-FileHash -LiteralPath $consentPath -Algorithm SHA256).Hash
function Verify-TestConsent {
  $saved=Get-Content -LiteralPath $consentPath -Raw | ConvertFrom-Json
  if ($saved.version -ne $consentVersion -or $saved.answered -le 0 -or
      $saved.choices.performance -cne $false -or $saved.choices.crashes -cne $false -or
      $saved.choices.bugs -cne $false -or $saved.choices.benchmarks -cne $false -or
      -not [string]::IsNullOrEmpty($saved.install_id) -or
      (Get-FileHash -LiteralPath $consentPath -Algorithm SHA256).Hash -ne $consentHash) {
    throw 'Test-profile all-off consent missing or changed'
  }
}
Verify-TestConsent
"test-consent: preseeded Keep optional data off equivalent; schema=$consentVersion; all four optional categories=false; install_id absent; basic_summaries=false; test profile only" | Add-Content $info
$env:HORIZON_CAPTURE_CLIENT_REVIEW=$env:CAP_ROUTE
Remove-Item Env:HORIZON_LAUNCHER_DESKTOP_ENTRY -ErrorAction SilentlyContinue

# Identity ledger: PID alone never authorizes observation, input or cleanup.
$owned = @{}
$script:survivor = $false
function Birth($p) { return $p.StartTime.ToUniversalTime().Ticks }
function Track($p,$kind) {
  $born=Birth $p
  $owned[[string]$p.Id]=@{Pid=$p.Id;Birth=$born;Kind=$kind}
  "owned-process: pid=$($p.Id) birth=$born kind=$kind" | Add-Content $info
}
function Same($o) {
  $p=Get-Process -Id $o.Pid -ErrorAction SilentlyContinue
  if ($null -eq $p) { return $null }
  try { if ((Birth $p) -eq $o.Birth) { return $p } } catch { return $null }
  return $null
}
function Descendants {
  # Snapshot descendants while each parent is still verifiably the recorded process.
  # A process born/exited between snapshots can be missed; no exhaustive-tree claim.
  $rows=@(Get-CimInstance Win32_Process -ErrorAction Stop)
  if ($rows.Count -gt 4096) { throw 'Process snapshot exceeds ceiling' }
  for ($pass=0; $pass -lt 8; $pass++) {
    $added=$false
    foreach ($row in $rows) {
      $key=[string]$row.ProcessId; $parent=$owned[[string]$row.ParentProcessId]
      if ($owned.ContainsKey($key) -or $null -eq $parent -or $null -eq (Same $parent)) { continue }
      $child=Get-Process -Id $row.ProcessId -ErrorAction SilentlyContinue
      if ($null -eq $child) { continue }
      try {
        $birth=Birth $child
        if ($birth -lt $parent.Birth -or [Math]::Abs($birth - $row.CreationDate.ToUniversalTime().Ticks) -gt 10) { continue }
        Track $child 'observed-descendant'; $added=$true
        if ($owned.Count -gt 64) { throw 'Owned process ceiling' }
      } catch { throw 'Descendant birth verification failed' }
    }
    if (-not $added) { break }
    if ($pass -eq 7) { throw 'Descendant depth ceiling' }
  }
}
function Verify-Window {
  $live=Same $owned[[string]$proc.Id]
  if ($null -eq $live) { throw 'Launcher identity no longer live' }
  $live.Refresh(); $h=$live.MainWindowHandle; [uint32]$ownerPid=0
  if ($h -eq [IntPtr]::Zero) { throw 'No owned window' }
  [WinCap]::GetWindowThreadProcessId($h,[ref]$ownerPid) | Out-Null
  if ($ownerPid -ne $proc.Id -or $null -eq (Same $owned[[string]$ownerPid])) { throw 'Window PID/birth mismatch' }
  $r=New-Object WinCap+RECT; $c=New-Object WinCap+RECT
  if (-not [WinCap]::GetWindowRect($h,[ref]$r) -or -not [WinCap]::GetClientRect($h,[ref]$c) -or -not [WinCap]::IsWindowVisible($h) -or [WinCap]::IsIconic($h) -or [WinCap]::GetDpiForWindow($h) -ne 96) { throw 'Window geometry/visibility/DPI invalid' }
  return @{Handle=$h;Outer=$r;Client=$c}
}
function Cleanup-Owned {
  try { Descendants } catch { 'cleanup: descendant snapshot unverified' | Add-Content $info; $script:survivor=$true }
  foreach ($o in @($owned.Values)) {
    $live=Same $o
    if ($null -eq $live) { continue }
    # Revalidate immediately before an owned-process graceful close request.
    $live=Same $o
    if ($null -ne $live) {
      try { $requested=$live.CloseMainWindow(); "close-request: pid=$($o.Pid) birth=$($o.Birth) accepted=$requested" | Add-Content $info }
      catch { "close-request: pid=$($o.Pid) error-type=$($_.Exception.GetType().Name)" | Add-Content $info }
    }
  }
  $until=[DateTime]::UtcNow.AddSeconds(8)
  do {
    $remaining=@($owned.Values | Where-Object { $null -ne (Same $_) })
    if ($remaining.Count -eq 0) { break }
    Start-Sleep -Milliseconds 200
    try { Descendants } catch { $script:survivor=$true }
  } while ([DateTime]::UtcNow -lt $until)
  foreach ($o in @($owned.Values)) {
    if ($null -ne (Same $o)) { "SURVIVOR: pid=$($o.Pid) birth=$($o.Birth) kind=$($o.Kind)" | Add-Content $info; $script:survivor=$true }
  }
  if ($script:survivor) { Set-Content (Join-Path $env:RUNNER_TEMP 'client-capture-stop') 'survivor or identity uncertainty, no further routes' }
}
$proc=$null
try {
  $proc=Start-Process -FilePath $bin -PassThru -RedirectStandardOutput "$env:CAP\launcher.stdout" -RedirectStandardError "$env:CAP\launcher.stderr"
  Track $proc 'launcher'
  $ready=$false
  for ($i=0;$i -lt 120;$i++) {
    if ($null -eq (Same $owned[[string]$proc.Id])) { throw 'Launcher exited before window' }
    try { $v=Verify-Window; $ready=$true; break } catch { Start-Sleep -Milliseconds 500 }
  }
  if (-not $ready) { throw 'No verified window within 60 seconds' }
  $dw=($v.Outer.Right-$v.Outer.Left)-$v.Client.Right; $dh=($v.Outer.Bottom-$v.Outer.Top)-$v.Client.Bottom
  $v=Verify-Window
  if ([int]$env:CAP_WIDTH -ge 1920) { [WinCap]::SetWindowPos($v.Handle,[IntPtr]::Zero,0,0,([int]$env:CAP_WIDTH+$dw),([int]$env:CAP_HEIGHT+$dh),0x0544) | Out-Null }
  else { [WinCap]::MoveWindow($v.Handle,0,0,([int]$env:CAP_WIDTH+$dw),([int]$env:CAP_HEIGHT+$dh),$true) | Out-Null }
  $steps=@(@{Name='top';Delta=0},@{Name='scroll-down-1';Delta=-900},@{Name='scroll-down-2';Delta=-900},@{Name='scroll-up';Delta=1800})
  $f=0
  foreach ($step in $steps) {
    $f++; Descendants
    Verify-TestConsent
    if ($step.Delta) {
      $v=Verify-Window
      if ($v.Client.Right -ne [int]$env:CAP_WIDTH -or $v.Client.Bottom -ne [int]$env:CAP_HEIGHT) { throw 'Scroll client mismatch' }
      $sx=$v.Outer.Left+[Math]::Min(850,$v.Client.Right-50); $sy=$v.Outer.Top+[Math]::Min(500,$v.Client.Bottom-80)
      if (-not [System.Windows.Forms.SystemInformation]::VirtualScreen.Contains([int]$sx,[int]$sy)) { throw 'Scroll point outside desktop' }
      [WinCap]::SetForegroundWindow($v.Handle) | Out-Null
      Start-Sleep -Milliseconds 300
      if (-not [WinCap]::SetCursorPos($sx,$sy)) { throw 'Cannot position cursor' }
      Start-Sleep -Milliseconds 300
      $now=Verify-Window
      $point=New-Object WinCap+POINT
      if (-not [WinCap]::GetCursorPos([ref]$point)) { throw 'Cannot read cursor' }
      $at=[WinCap]::WindowFromPoint($point); $rootWindow=[WinCap]::GetAncestor($at,2); [uint32]$atPid=0
      [WinCap]::GetWindowThreadProcessId($at,[ref]$atPid) | Out-Null
      "scroll-routing: cursor=($($point.X),$($point.Y)) target=($sx,$sy) point-hwnd=$at root-hwnd=$rootWindow point-pid=$atPid launcher-pid=$($proc.Id); non-atomic checks" | Add-Content $info
      if ($point.X -ne $sx -or $point.Y -ne $sy -or $rootWindow -ne $now.Handle -or $atPid -ne $proc.Id -or [WinCap]::GetForegroundWindow() -ne $now.Handle -or $now.Client.Right -ne [int]$env:CAP_WIDTH -or $now.Client.Bottom -ne [int]$env:CAP_HEIGHT) { throw 'Scroll routing changed' }
      if ($null -eq (Same $owned[[string]$proc.Id])) { throw 'Scroll birth mismatch' }
      $delta=[BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$step.Delta),0)
      [WinCap]::mouse_event(2048,0,0,$delta,[UIntPtr]::Zero)
    }
    Start-Sleep -Seconds $(if ($f -eq 1) {8} else {2})
    $v=Verify-Window
    if ($v.Client.Right -ne [int]$env:CAP_WIDTH -or $v.Client.Bottom -ne [int]$env:CAP_HEIGHT) { throw 'Capture client mismatch' }
    $width=$v.Outer.Right-$v.Outer.Left; $height=$v.Outer.Bottom-$v.Outer.Top
    if ($width -lt 200 -or $height -lt 200 -or $width -gt 3840 -or $height -gt 3840 -or [int64]$width*$height -gt 16777216) { throw 'Capture size ceiling' }
    "viewport: frame=$f outer=${width}x${height} client=$($v.Client.Right)x$($v.Client.Bottom) dpi=96 pid=$($proc.Id) birth=$($owned[[string]$proc.Id].Birth)" | Add-Content $info
    $png=Join-Path $env:CAP ("frame-{0:D2}-{1}.png" -f $f,$step.Name)
    # PrintWindow runs only in its own recorded worker, never in this controller.
    # Timeout cannot guarantee a hung native worker exits gracefully. Any survivor stops the run.
    $workerArgs=@('-NoProfile','-File',"$PSScriptRoot\client-review-worker.ps1",'-WindowHandle',[string]$v.Handle,'-LauncherPid',[string]$proc.Id,'-LauncherBirth',[string]$owned[[string]$proc.Id].Birth,'-Width',[string]$width,'-Height',[string]$height,'-Output',$png)
    $quotedArgs=($workerArgs | ForEach-Object { '"' + ([string]$_).Replace('"','\"') + '"' }) -join ' '
    $worker=Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList $quotedArgs -PassThru -RedirectStandardOutput "$env:CAP\worker-$f.stdout" -RedirectStandardError "$env:CAP\worker-$f.stderr"
    Track $worker 'capture-worker'
    $end=[DateTime]::UtcNow.AddSeconds(30)
    while ($null -ne (Same $owned[[string]$worker.Id]) -and [DateTime]::UtcNow -lt $end) { Descendants; Start-Sleep -Milliseconds 200 }
    if ($null -ne (Same $owned[[string]$worker.Id])) { throw 'Capture worker exceeded 30-second bound' }
    $worker.WaitForExit()
    if ($worker.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $png)) { throw 'Capture worker failed' }
    $v=Verify-Window
  }
  Verify-TestConsent
  'AUTOMATION-CHECKS: 4 originals saved; instrumented fixture; pixels/scroll behavior still require review' | Add-Content $info
  'LIMITS: non-atomic foreground/cursor/input checks; descendants can be missed between snapshots; no zero-network or shipping-tip claim' | Add-Content $info
} catch {
  # Never print an unbounded exception object, filename or window title.
  "FAIL: error-type=$($_.Exception.GetType().Name)" | Add-Content $info
  throw 'Capture route failed; bounded receipts retained'
} finally { Cleanup-Owned }
if ($script:survivor) { throw 'Owned process survived graceful cleanup; further routes stopped' }
