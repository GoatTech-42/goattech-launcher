param([Parameter(Mandatory=$true)][int]$OwnedPid,[Parameter(Mandatory=$true)][string]$Out)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class NativeBoundaryCapture {
 public delegate bool EnumProc(IntPtr w,IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f,IntPtr p);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr w);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr w,out uint p);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr w,out Rect r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr w,IntPtr dc,uint f);
 public struct Rect { public int L,T,R,B; }
}
'@
$p=[Diagnostics.Process]::GetProcessById($OwnedPid)
try {
 if ($p.HasExited) {throw 'Owned fixture is not alive'}
 $script:w=[IntPtr]::Zero
 [NativeBoundaryCapture]::EnumWindows({param($w,$x) [uint32]$id=0; [void][NativeBoundaryCapture]::GetWindowThreadProcessId($w,[ref]$id); if ($id -eq $OwnedPid -and [NativeBoundaryCapture]::IsWindowVisible($w)) {$script:w=$w;return $false};return $true},[IntPtr]::Zero) | Out-Null
 if ($script:w -eq [IntPtr]::Zero) {throw 'No visible owned fixture window'}
 $r=New-Object NativeBoundaryCapture+Rect
 if (-not [NativeBoundaryCapture]::GetWindowRect($script:w,[ref]$r)) {throw 'No owned window geometry'}
 $width=$r.R-$r.L;$height=$r.B-$r.T
 if ($width -lt 300 -or $width -gt 1800 -or $height -lt 100 -or $height -gt 1000) {throw 'Unexpected fixture geometry'}
 $b=New-Object Drawing.Bitmap($width,$height)
 try {$g=[Drawing.Graphics]::FromImage($b);try {$dc=$g.GetHdc();try {if (-not [NativeBoundaryCapture]::PrintWindow($script:w,$dc,2)) {throw 'Capture API refused'}} finally {$g.ReleaseHdc($dc)}} finally {$g.Dispose()};$b.Save($Out,[Drawing.Imaging.ImageFormat]::Png)}finally{$b.Dispose()}
 [uint32]$after=0;[void][NativeBoundaryCapture]::GetWindowThreadProcessId($script:w,[ref]$after)
 if ($after -ne $OwnedPid -or $p.HasExited) {throw 'Owned fixture changed during capture'}
 if ((Get-Item -LiteralPath $Out).Length -gt 1000000) {throw 'Capture too large'}
} finally {$p.Dispose()}
