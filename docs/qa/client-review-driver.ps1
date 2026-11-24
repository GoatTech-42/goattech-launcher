$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $env:CAP | Out-Null
$info = "$env:CAP\window-info.txt"
$src = 'using System;using System.Runtime.InteropServices;public class WinCap{[DllImport("user32.dll",SetLastError=true)]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);[DllImport("user32.dll")]public static extern IntPtr GetLastActivePopup(IntPtr h);[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]public static extern uint GetDpiForWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool GetClientRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);[DllImport("user32.dll")]public static extern bool IsIconic(IntPtr h);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h2,bool r);[DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);[DllImport("user32.dll")]public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);public struct RECT{public int Left,Top,Right,Bottom;}}'
if (-not ('WinCap' -as [type])) { Add-Type -TypeDefinition $src -ReferencedAssemblies System.Drawing }
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$bin = "$PWD\launcher\target\release\horizon-launcher.exe"
$actual = (git rev-parse HEAD).Trim()
if ($actual -ne 'cd5d9ce4b7c2e4f0e9ce0c4e28490c0fc91b6e1e') { throw 'Unexpected instrumented source' }
"source-commit: $actual; shipping-source: 235b8a9b7d920e36704b508b4136808976e91287; instrumented fixture, NOT exact shipping-tip evidence" | Out-File $info
"workflow-commit: $env:GITHUB_SHA" | Out-File $info -Append
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
$env:HORIZON_CAPTURE_CLIENT_REVIEW=$env:CAP_ROUTE
Remove-Item Env:HORIZON_LAUNCHER_DESKTOP_ENTRY -ErrorAction SilentlyContinue
$proc = Start-Process -FilePath $bin -PassThru -RedirectStandardOutput "$env:CAP\launcher.stdout" -RedirectStandardError "$env:CAP\launcher.stderr"
$checksPass = $false
try {
  # Bounded wait for the process's main window (Process.MainWindowHandle:
  # no EnumWindows callback, no delegate binding).
  $hwnd = [IntPtr]::Zero
  for ($i = 0; $i -lt 1200 -and $hwnd -eq [IntPtr]::Zero; $i++) {
    Start-Sleep -Milliseconds 50
    $proc.Refresh()
    if ($proc.HasExited) { throw "FAIL: launcher exited before a window (code $($proc.ExitCode))" }
    $h = $proc.MainWindowHandle
    if ($h -ne [IntPtr]::Zero -and [WinCap]::IsWindowVisible($h) -and -not [WinCap]::IsIconic($h)) {
      $r = New-Object WinCap+RECT
      if ([WinCap]::GetWindowRect($h, [ref]$r) -and ($r.Right - $r.Left) -ge 200 -and ($r.Bottom - $r.Top) -ge 200) {
        $hwnd = $h
        "initial-window: $($r.Right - $r.Left)x$($r.Bottom - $r.Top) hwnd=$hwnd title='$($proc.MainWindowTitle)'" | Out-File $info -Append
      }
    }
  }
  if ($hwnd -eq [IntPtr]::Zero) { throw "FAIL: no visible owned main window >=200x200 within 60s" }
  # Three capture attempts. Window state is re-validated and recorded per
  # frame; each PrintWindow runs in a child job with a BEST-EFFORT 30s
  # wait (see header: Stop-Job on a native hang may itself block).
  $okCount = 0; $seenCount = 0
  $savedSteps = @{}
  function Size-Client($width,$height) {
    $h=$proc.MainWindowHandle
    $outer=New-Object WinCap+RECT; $client=New-Object WinCap+RECT
    [WinCap]::GetWindowRect($h,[ref]$outer) | Out-Null
    [WinCap]::GetClientRect($h,[ref]$client) | Out-Null
    $dw=($outer.Right-$outer.Left)-$client.Right; $dh=($outer.Bottom-$outer.Top)-$client.Bottom
    if ($width -ge 1920) {
      $sized=[WinCap]::SetWindowPos($h,[IntPtr]::Zero,0,0,($width+$dw),($height+$dh),0x0544)
      "resize-method: oversized-native NOSENDCHANGING result=$sized requested-client=${width}x${height}" | Out-File $info -Append
    } else {
      [WinCap]::MoveWindow($h,0,0,($width+$dw),($height+$dh),$true) | Out-Null
    }
  }
  Size-Client ([int]$env:CAP_WIDTH) ([int]$env:CAP_HEIGHT)
  $steps=@(@{n='top';w=8},@{n='scroll-down-1';delta=-900;w=2},@{n='scroll-down-2';delta=-900;w=2},@{n='scroll-up';delta=1800;w=2})
  $expectedClient=@([int]$env:CAP_WIDTH,[int]$env:CAP_HEIGHT)
  'input: scroll only; no click, keyboard, file picker, installation or game execution' | Out-File $info -Append
  for ($f = 1; $f -le $steps.Count; $f++) {
    $st = $steps[$f-1]
    if ($st.r) { $expectedClient = $st.r; Size-Client $st.r[0] $st.r[1] }
    if ($st.delta) {
      if ($savedSteps.Count -eq 0) { throw 'Scroll requires saved original' }
      $h=$proc.MainWindowHandle; $pr=New-Object WinCap+RECT; $pc=New-Object WinCap+RECT
      if (-not [WinCap]::GetWindowRect($h,[ref]$pr) -or -not [WinCap]::GetClientRect($h,[ref]$pc)) { throw 'Cannot verify scroll geometry' }
      if ($pc.Right -ne $expectedClient[0] -or $pc.Bottom -ne $expectedClient[1] -or [WinCap]::GetDpiForWindow($h) -ne 96 -or -not [WinCap]::IsWindowVisible($h) -or [WinCap]::IsIconic($h)) { throw 'Scroll geometry invalid' }
      $sx=$pr.Left+[Math]::Min(850,$pc.Right-50); $sy=$pr.Top+[Math]::Min(500,$pc.Bottom-80)
      if (-not [System.Windows.Forms.SystemInformation]::VirtualScreen.Contains([int]$sx,[int]$sy)) { throw 'Scroll outside desktop' }
      [WinCap]::SetForegroundWindow($h) | Out-Null
      Start-Sleep -Milliseconds 300
      if (-not [WinCap]::SetCursorPos($sx,$sy)) { throw 'Cannot position pointer' }
      Start-Sleep -Milliseconds 300
      if ([WinCap]::GetForegroundWindow() -ne $h) { throw 'Scroll requires owned foreground' }
      $now=New-Object WinCap+RECT; $outer=New-Object WinCap+RECT
      if (-not [WinCap]::GetClientRect($h,[ref]$now) -or -not [WinCap]::GetWindowRect($h,[ref]$outer) -or $now.Right -ne $pc.Right -or $now.Bottom -ne $pc.Bottom -or $outer.Left -ne $pr.Left -or $outer.Top -ne $pr.Top -or [WinCap]::GetDpiForWindow($h) -ne 96) { throw 'Scroll geometry changed' }
      "scroll: target=($sx,$sy) delta=$($st.delta) client=$($pc.Right)x$($pc.Bottom)" | Out-File $info -Append
      $delta=[BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$st.delta),0)
      [WinCap]::mouse_event(2048,0,0,$delta,[UIntPtr]::Zero)
    }
    if ($st.ms) { Start-Sleep -Milliseconds $st.ms } else { Start-Sleep -Seconds $st.w }
    $proc.Refresh()
    if ($proc.HasExited) { throw "FAIL: launcher died before frame $f (code $($proc.ExitCode))" }
    $h = $proc.MainWindowHandle
    if ($h -eq [IntPtr]::Zero) { "frame ${f}: SKIPPED main window handle lost" | Out-File $info -Append; continue }
    $r = New-Object WinCap+RECT
    $rectOk = [WinCap]::GetWindowRect($h, [ref]$r)
    $w = if ($rectOk) { $r.Right - $r.Left } else { 0 }
    $hgt = if ($rectOk) { $r.Bottom - $r.Top } else { 0 }
    $vis = [WinCap]::IsWindowVisible($h); $iconic = [WinCap]::IsIconic($h)
    $state = "visible=$vis iconic=$iconic geometry=${w}x${hgt}"
    if (-not $rectOk -or -not $vis -or $iconic -or $w -lt 200 -or $hgt -lt 200) {
      "frame ${f}: WINDOW-NOT-OK $state - capture skipped" | Out-File $info -Append; continue
    }
    # Geometry/pixel ceiling BEFORE any bitmap allocation: an absurd
    # window must not allocate an unbounded bitmap.
    if ($w -gt 3840 -or $hgt -gt 3840 -or [int64]$w * $hgt -gt 16777216) {
      "frame ${f}: CAPTURE-SKIPPED-OVERSIZE $state (limits 3840px/16MP)" | Out-File $info -Append; continue
    }
    $seenCount++
    $png = "$env:CAP\frame-$($f.ToString('00'))-$($st.n).png"
    $client=New-Object WinCap+RECT
    [WinCap]::GetClientRect($h,[ref]$client) | Out-Null
    $dpi = [WinCap]::GetDpiForWindow($h)
    "viewport: frame=$f outer=${w}x${hgt} client=$($client.Right)x$($client.Bottom) dpi=$dpi" | Out-File $info -Append
    if ($dpi -ne 96) { throw 'Unexpected capture DPI; stop for recalibration' }
    if ($client.Right -ne $expectedClient[0] -or $client.Bottom -ne $expectedClient[1]) { throw 'Unexpected client geometry for requested frame' }
    $bmp=New-Object System.Drawing.Bitmap $w,$hgt
    $g=[System.Drawing.Graphics]::FromImage($bmp); $dc=[IntPtr]::Zero
    try {
      $dc=$g.GetHdc(); $ok=[WinCap]::PrintWindow($h,$dc,2)
      $g.ReleaseHdc($dc); $dc=[IntPtr]::Zero
      if (-not $ok) { throw 'PrintWindow refused capture' }
      $bmp.Save($png,[System.Drawing.Imaging.ImageFormat]::Png)
      $okCount++
      $savedSteps[$st.n] = $png
    } finally {
      if ($dc -ne [IntPtr]::Zero) { $g.ReleaseHdc($dc) }
      $g.Dispose();$bmp.Dispose()
    }
  }
  $proc.Refresh()
  if ($proc.HasExited) { throw "FAIL: launcher exited during captures (code $($proc.ExitCode))" }
  # Automation checks only. Window claim is what the per-frame lines show.
  "AUTOMATION-CHECKS: process-alive=whole-phase initial-visible-owned-window>=200x200=yes frames-window-ok=$seenCount/$($steps.Count) captures-accepted=$okCount/$($steps.Count)" | Out-File $info -Append
  if ($seenCount -eq 0) { "WINDOW-AFTER-INITIAL: unverified (every frame skipped)" | Out-File $info -Append }
  "NOT-PROVEN: rendering/pixels/input/consent/game-launch/GPU/continuous-presence - frames require human inspection" | Out-File $info -Append
  $required = @($steps | ForEach-Object { $_.n })
  foreach ($name in $required) {
    if (-not $savedSteps.ContainsKey($name) -or -not (Test-Path -LiteralPath $savedSteps[$name]) -or (Get-Item -LiteralPath $savedSteps[$name]).Length -eq 0) {
      throw "Required saved capture missing or empty: $name"
    }
  }
  if ($okCount -ne $steps.Count) { throw "Saved capture count $okCount does not match requested step count $($steps.Count)" }
  'NOT-PROVEN: declaration/scroll pixels until review; foreground/input race remains. No keyboard or click input.' | Out-File $info -Append
  $checksPass = $true
} finally {
  # Best-effort cleanup: any leftover capture jobs, then the launcher's main
  # process only - children are not tracked and no tree cleanup is claimed.
  Get-Job | Stop-Job -ErrorAction SilentlyContinue | Out-Null
  Get-Job | Remove-Job -Force -ErrorAction SilentlyContinue
  if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}
if (-not $checksPass) { throw 'Route capture checks failed' }
