param([long]$WindowHandle,[int]$LauncherPid,[long]$LauncherBirth,[int]$Width,[int]$Height,[string]$Output)
$ErrorActionPreference='Stop'
$src='using System;using System.Runtime.InteropServices;public class WorkerCap{[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr a,int x,int y,int w,int z,uint f);[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();}'
Add-Type -TypeDefinition $src
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$p=Get-Process -Id $LauncherPid
if ($p.StartTime.ToUniversalTime().Ticks -ne $LauncherBirth) { throw 'Birth mismatch' }
[uint32]$ownerPid=0; $h=[IntPtr]$WindowHandle
[WorkerCap]::GetWindowThreadProcessId($h,[ref]$ownerPid) | Out-Null
if ($ownerPid -ne $LauncherPid) { throw 'Window ownership mismatch' }
if ($Width -lt 200 -or $Height -lt 200 -or $Width -gt 3840 -or $Height -gt 3840 -or [long]$Width*$Height -gt 16777216) { throw 'Geometry ceiling' }
$screen=[System.Windows.Forms.SystemInformation]::VirtualScreen
if ($screen.Width -gt 3840 -or $screen.Height -gt 2160) {throw 'Desktop ceiling'}
$bmp=[System.Drawing.Bitmap]::new($screen.Width,$screen.Height);$g=[System.Drawing.Graphics]::FromImage($bmp);$dc=[IntPtr]::Zero
try {
  $p=Get-Process -Id $LauncherPid
  if ($p.StartTime.ToUniversalTime().Ticks -ne $LauncherBirth) { throw 'Birth changed' }
  [WorkerCap]::GetWindowThreadProcessId($h,[ref]$ownerPid) | Out-Null
  if ($ownerPid -ne $LauncherPid) { throw 'Window owner changed' }
  # Harness-only: keep the owned launcher above the CI terminal before sampling.
  if (-not [WorkerCap]::SetWindowPos($h,[IntPtr](-1),0,0,0,0,0x0043)) {throw 'Cannot raise owned launcher'}
  [WorkerCap]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 750
  if ([WorkerCap]::GetForegroundWindow() -ne $h) {throw 'Owned launcher is not foreground'}
  $g.CopyFromScreen($screen.Left,$screen.Top,0,0,$screen.Size)
  $bmp.Save($Output,[System.Drawing.Imaging.ImageFormat]::Png)
} finally { if ($dc -ne [IntPtr]::Zero) {$g.ReleaseHdc($dc)};$g.Dispose();$bmp.Dispose() }
