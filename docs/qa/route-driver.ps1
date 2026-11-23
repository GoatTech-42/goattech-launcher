$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $env:CAP | Out-Null
$info = "$env:CAP\window-info.txt"
# C# as ONE single-quoted line: no here-string anywhere in this file.
$src = 'using System;using System.Runtime.InteropServices;public class WinCap{[DllImport("user32.dll",SetLastError=true)]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);[DllImport("user32.dll")]public static extern IntPtr GetLastActivePopup(IntPtr h);[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]public static extern uint GetDpiForWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool GetClientRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);[DllImport("user32.dll")]public static extern bool IsIconic(IntPtr h);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h2,bool r);[DllImport("user32.dll")]public static extern bool SetCursorPos(int x,int y);[DllImport("user32.dll")]public static extern void mouse_event(uint f,uint dx,uint dy,uint d,UIntPtr e);[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);public struct RECT{public int Left,Top,Right,Bottom;}}'
if (-not ('WinCap' -as [type])) { Add-Type -TypeDefinition $src -ReferencedAssemblies System.Drawing }
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$bin = "$PWD\launcher\target\release\horizon-launcher.exe"
$binHash = (Get-FileHash $bin -Algorithm SHA256).Hash
$actual = (git rev-parse HEAD).Trim()
if ($actual -ne 'f1fa45bba5e0835e6cabb1faf7be30a29591d5ed') { throw 'Unexpected capture source' }
if ($env:CAP_KIND -ne 'exact') { throw 'This job permits exact source only' }
"workflow-commit: $env:GITHUB_SHA" | Out-File $info
"source-commit: $actual" | Out-File $info -Append
          "capture-kind: $env:CAP_KIND route=$env:CAP_ROUTE; instrumented frames are not exact-tip evidence" | Out-File $info -Append
"rustc: $(rustc --version)" | Out-File $info -Append
"cargo: $(cargo --version)" | Out-File $info -Append
"os: $([System.Environment]::OSVersion.VersionString)" | Out-File $info -Append
"runner-image: $env:ImageOS $env:ImageVersion" | Out-File $info -Append
"binary-sha256: $binHash" | Out-File $info -Append
if ($env:CAP_KIND -eq 'instrumented') { "seam-sha256: $((Get-FileHash (Join-Path $PSScriptRoot 'route-capture-only.patch') -Algorithm SHA256).Hash)" | Out-File $info -Append }
New-Item -ItemType Directory -Force $env:HORIZON_LAUNCHER_HOME | Out-Null
@{install_new_versions=$false;skip_opening=($env:CAP_MODE -ne 'opening');selected='v2026.10.15'} | ConvertTo-Json | Set-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'settings.json')
$fake = Join-Path $env:HORIZON_LAUNCHER_HOME 'versions/v2026.10.15'
New-Item -ItemType Directory -Force $fake | Out-Null
'' | Set-Content (Join-Path $fake '.installed')
'{"minecraft":{"version":"26.3"}}' | Set-Content (Join-Path $fake 'research-lock.json')
$binaryText = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($bin))
if ($binaryText.Contains('my.sculkhosting.com/ref/horizon') -or $binaryText.Contains('HORIZON15')) { throw 'Referral payload found in build' }
'sponsor-check: referral and promo strings absent; synthetic offline fixture; no runnable game/accounts' | Out-File $info -Append
if ($true) {
  # Synthetic, local-only fixtures. Never add Microsoft accounts or credentials.
  $settings = @{install_new_versions=$false;skip_opening=$true;selected='v2026.10.15';owner_notice_seen=$true;ping_notice_seen=$true;basic_summaries=$false}
  $settings | ConvertTo-Json | Set-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'settings.json')
  $env:HORIZON_LAUNCHER_PAGE = $env:CAP_ROUTE
            if ($env:CAP_ROUTE -in @('export','launch','skin')) { $env:HORIZON_LAUNCHER_PAGE = if ($env:CAP_ROUTE -eq 'skin') {'settings'} else {'home'} }
            & "$PSScriptRoot/route-fixtures.ps1"
            if ($env:CAP_KIND -eq 'instrumented') { $env:GOAT_CAPTURE_ROUTE = $env:CAP_ROUTE } else { Remove-Item Env:GOAT_CAPTURE_ROUTE -ErrorAction SilentlyContinue }
  if ($env:PROBE_STATE -ne 'consent') {
    $telemetry = Join-Path $env:HORIZON_HOME 'telemetry'
    New-Item -ItemType Directory -Force $telemetry | Out-Null
    # Read the reviewed source's disclosure version, never bypass a new disclosure by guess.
    $match = [regex]::Match((Get-Content 'crates/horizon-telemetry/src/lib.rs' -Raw), 'pub const CONSENT_VERSION: u32 = ([0-9]+);')
    if (-not $match.Success) { throw 'Consent fixture schema unknown' }
    @{version=[int]$match.Groups[1].Value;answered=1;choices=@{performance=$false;crashes=$false;bugs=$false;benchmarks=$false}} | ConvertTo-Json | Set-Content (Join-Path $telemetry 'consent.json')
  }
  @{accounts=@();offline=@('Capture_Player','Capture_Alt');offline_selected='Capture_Player'} | ConvertTo-Json | Set-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'accounts.json')
  $shots = Join-Path $env:HORIZON_HOME 'screenshots'
  New-Item -ItemType Directory -Force $shots | Out-Null
  $fixture = New-Object System.Drawing.Bitmap 640,360
  $paint = [System.Drawing.Graphics]::FromImage($fixture)
  try { $paint.Clear([System.Drawing.Color]::SlateBlue); $fixture.Save((Join-Path $shots 'synthetic-capture-long-filename-2026-11-23.png'),[System.Drawing.Imaging.ImageFormat]::Png) }
  finally { $paint.Dispose(); $fixture.Dispose() }
  $portrait = New-Object System.Drawing.Bitmap 240,640
  $paint2 = [System.Drawing.Graphics]::FromImage($portrait)
  try { $paint2.Clear([System.Drawing.Color]::DarkOrange); $portrait.Save((Join-Path $shots 'synthetic-portrait-second-image-with-a-deliberately-long-filename-for-viewer-layout-2026-11-23.png'),[System.Drawing.Imaging.ImageFormat]::Png) }
  finally { $paint2.Dispose(); $portrait.Dispose() }
  $env:HORIZON_LAUNCHER_TRASH = Join-Path $env:CAP 'synthetic-trash'
  $env:HORIZON_LAUNCHER_CLIPBOARD = Join-Path $env:CAP 'synthetic-clipboard'
  $env:HORIZON_LAUNCHER_DESKTOP = Join-Path $env:CAP 'synthetic-desktop'
  "probe-request: $env:PROBE_STATE; fixtures local/synthetic; state unverified until pixel review" | Out-File $info -Append
}
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
  if ($env:CAP_MODE -eq 'opening') {
    Size-Client ([int]$env:CAP_WIDTH) ([int]$env:CAP_HEIGHT)
    $steps=@(@{n='opening-early'; ms=150},@{n='opening-held'; ms=250},@{n='opening-late'; ms=350})
  } else {
    $steps = @(
      @{n='dialog'; w=8},
      @{n='sign-in-prompt'; c=@(500,649); w=3},
      @{n='account-dismissed'; k='{ESC}'; w=2},
      @{n='page-minimum'; r=@(960,600); w=3},
      @{n='page-1028x780'; r=@(1028,780); w=3},
      @{n='page-1024x720'; r=@(1024,720); w=3},
      @{n='large-1920x1080'; r=@(1920,1080); w=5},
      @{n='large-2560x1440'; r=@(2560,1440); w=5}
    )
  }
  if ($true) {
    Size-Client 1028 780
    # Calibration only: no pointer, keyboard, or drag input.
    $inputSteps = @()
    $steps = @(@{n='probe-fixture';w=8}) + $inputSteps + @(@{n='probe-1028x780';w=2})
              $steps += @{n='probe-960x600';r=@(960,600);w=3}
              $steps += @{n='probe-2560x1440';r=@(2560,1440);w=4}
              $steps += @{n='scroll-lower';s=@(850,600,-900);w=2}
    'probe-input: calibration-only geometry and lower-scroll; no pointer click or keyboard input' | Out-File $info -Append
  }
  for ($f = 1; $f -le $steps.Count; $f++) {
    $st = $steps[$f-1]
    if ($st.r) { Size-Client $st.r[0] $st.r[1] }
    if ($st.s) { $pr = New-Object WinCap+RECT; [WinCap]::GetWindowRect($proc.MainWindowHandle, [ref]$pr) | Out-Null; [WinCap]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null; Start-Sleep -Milliseconds 300; [WinCap]::SetCursorPos($pr.Left + $st.s[0], $pr.Top + $st.s[1]) | Out-Null; Start-Sleep -Milliseconds 300; $d = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int]$st.s[2]),0); [WinCap]::mouse_event(2048,0,0,$d,[UIntPtr]::Zero) }
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
    "viewport: frame=$f client=$($client.Right)x$($client.Bottom) dpi=$([WinCap]::GetDpiForWindow($h))" | Out-File $info -Append
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
  $required = @('account-dismissed','page-minimum','page-1028x780','page-1024x720','large-1920x1080','large-2560x1440')
  if ($true) { $required = @('probe-fixture','probe-1028x780','probe-960x600','probe-2560x1440','scroll-lower') }
  foreach ($name in $required) {
    if (-not $savedSteps.ContainsKey($name) -or -not (Test-Path -LiteralPath $savedSteps[$name]) -or (Get-Item -LiteralPath $savedSteps[$name]).Length -eq 0) {
      throw "Required saved capture missing or empty: $name"
    }
  }
  if ($okCount -ne $steps.Count) { throw "Saved capture count $okCount does not match requested step count $($steps.Count)" }
  'NOT-PROVEN: unobscured requested page or dismissed consent/account screens until pixel review; Escape may be consumed by consent if the earlier click missed. Requested names are labels only; use measured client size and DPI. Foreground check/send has a race; pointer clicks are coordinate-based.' | Out-File $info -Append
  $checksPass = $true
} finally {
  # Best-effort cleanup: any leftover capture jobs, then the launcher's main
  # process only - children are not tracked and no tree cleanup is claimed.
  Get-Job | Stop-Job -ErrorAction SilentlyContinue | Out-Null
  Get-Job | Remove-Job -Force -ErrorAction SilentlyContinue
  if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
}
if (-not $checksPass) { throw 'Route capture checks failed' }
