$ErrorActionPreference='Stop'
$source='cdbc87ee2a6d65e81555603be30b7690afb52c5f'
if ((git rev-parse HEAD).Trim() -ne $source) {throw 'Unexpected source'}
$evidence=Join-Path $env:RUNNER_TEMP 'import-gui-evidence';New-Item -ItemType Directory $evidence | Out-Null
$root=Join-Path $env:RUNNER_TEMP 'import-gui-profile';if (Test-Path $root) {throw 'Profile already exists'}
$roaming=[Environment]::GetFolderPath('ApplicationData');$prism=Join-Path $roaming 'PrismLauncher'
if (Test-Path $prism) {throw 'Existing Prism folder; do not overwrite'}
$packet=Join-Path $PSScriptRoot 'import-fixture'
$checks=[Collections.Generic.List[object]]::new();$actions=[Collections.Generic.List[object]]::new();$frames=[Collections.Generic.List[object]]::new();$script:seq=0;$script:proc=$null;$script:birth=0
function Check($name,$ok) { $checks.Add(@{name=$name;pass=[bool]$ok});if (-not $ok) {throw "Check failed: $name"} }
function HashTree($base) {return @(Get-ChildItem -LiteralPath $base -Recurse -File | Sort-Object FullName | ForEach-Object {@{path=$_.FullName.Substring($base.Length+1).Replace('\','/');bytes=$_.Length;sha256=(Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()}})}
function Json($o,$path) {ConvertTo-Json -InputObject $o -Depth 12 | Set-Content -LiteralPath $path}
Add-Type -AssemblyName System.Drawing,System.Windows.Forms,System.Runtime.WindowsRuntime
Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public class ImportNative {
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr a,int x,int y,int w,int z,uint f);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h,ref POINT p);
public struct POINT {public int X,Y;}
[DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h,out RECT r);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
public struct RECT {public int L,T,R,B;}
}
'@
[Windows.Storage.StorageFile,Windows.Storage,ContentType=WindowsRuntime] | Out-Null
[Windows.Graphics.Imaging.BitmapDecoder,Windows.Foundation,ContentType=WindowsRuntime] | Out-Null
[Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime] | Out-Null
[Windows.Globalization.Language,Windows.Globalization,ContentType=WindowsRuntime] | Out-Null
$script:ocr=[Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage([Windows.Globalization.Language]::new('en-US'))
if ($null -eq $script:ocr) {throw 'Windows OCR unavailable'}
$script:asTask=([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {$_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'})[0]
function Await($op,[Type]$t) { $task=$script:asTask.MakeGenericMethod($t).Invoke($null,@($op));if (-not $task.Wait(10000)) {throw 'WinRT timeout'};return $task.Result }
function OwnedWindow {
 $p=Get-Process -Id $script:proc.Id
 if ($p.StartTime.ToUniversalTime().Ticks -ne $script:birth) {throw 'Launcher birth changed'}
 $p.Refresh();$h=$p.MainWindowHandle;[uint32]$owner=0;[ImportNative]::GetWindowThreadProcessId($h,[ref]$owner)|Out-Null
 if ($h -eq [IntPtr]::Zero -or $owner -ne $p.Id) {throw 'No owned launcher window'}
 $r=New-Object ImportNative+RECT;[ImportNative]::GetWindowRect($h,[ref]$r)|Out-Null
 return @{h=$h;r=$r}
}
function Foreground {
 $v=OwnedWindow
 [ImportNative]::SetWindowPos($v.h,[IntPtr](-1),0,0,0,0,0x0043)|Out-Null;[ImportNative]::SetForegroundWindow($v.h)|Out-Null
 Start-Sleep -Milliseconds 400
 if ([ImportNative]::GetForegroundWindow() -ne $v.h) {throw 'Launcher not foreground'}
}
function Snap($label) {
 Foreground;$script:seq++;$name=('{0:D2}-{1}.png' -f $script:seq,$label);$path=Join-Path $evidence $name
 # OCR must never merge the launcher's clipped bottom line with another process behind it.
 $v=OwnedWindow;$client=New-Object ImportNative+RECT;$origin=New-Object ImportNative+POINT
 if (-not [ImportNative]::GetClientRect($v.h,[ref]$client) -or -not [ImportNative]::ClientToScreen($v.h,[ref]$origin)) {throw 'Client capture bounds unavailable'}
 $captureX=$origin.X;$captureY=$origin.Y
 $bmp=[Drawing.Bitmap]::new($client.R,$client.B);$g=[Drawing.Graphics]::FromImage($bmp)
 try {$g.CopyFromScreen($captureX,$captureY,0,0,$bmp.Size);$bmp.Save($path,[Drawing.Imaging.ImageFormat]::Png)} finally {$g.Dispose();$bmp.Dispose()}
 $file=Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($path)) ([Windows.Storage.StorageFile])
 $stream=Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
 try {
  $decoder=Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
  $soft=Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
  try {$result=Await ($script:ocr.RecognizeAsync($soft)) ([Windows.Media.Ocr.OcrResult])} finally {$soft.Dispose()}
 } finally {$stream.Dispose()}
 $lines=@($result.Lines | ForEach-Object { @{text=$_.Text;words=@($_.Words | ForEach-Object {@{text=$_.Text;x=($_.BoundingRect.X+$captureX);y=($_.BoundingRect.Y+$captureY);w=$_.BoundingRect.Width;h=$_.BoundingRect.Height}})} })
 Json $lines (Join-Path $evidence ($name+'.ocr.json'));$frames.Add(@{capture_x=$captureX;capture_y=$captureY;capture_width=$client.R;capture_height=$client.B;file=$name;sha256=(Get-FileHash $path -Algorithm SHA256).Hash;bytes=(Get-Item $path).Length})
 return @{lines=$lines;file=$name}
}
function ClickAt($x,$y,$why) {
 Foreground;$v=OwnedWindow
 if ($x -lt $v.r.L -or $x -ge $v.r.R -or $y -lt $v.r.T -or $y -ge $v.r.B) {throw 'Click outside owned launcher'}
 $actions.Add(@{action='click';x=$x;y=$y;reason=$why;at=[DateTime]::UtcNow.ToString('o')})
 [ImportNative]::SetCursorPos($x,$y)|Out-Null;[ImportNative]::mouse_event(2,0,0,0,[UIntPtr]::Zero);[ImportNative]::mouse_event(4,0,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 900
}
function Scroll($down) {Foreground;$actions.Add(@{action='scroll';down=$down});[ImportNative]::SetCursorPos(800,420)|Out-Null;$delta=if ($down) {[uint32]::MaxValue-599} else {[uint32]600};[ImportNative]::mouse_event(0x800,0,0,$delta,[UIntPtr]::Zero);Start-Sleep -Milliseconds 700}
function Target($pattern,$label,$scroll=$false) {
 for ($i=0;$i -lt $(if ($scroll) {6}else{1});$i++) {
  $s=Snap $label;$matches=@($s.lines | Where-Object {$_.text -match $pattern -and $_.words[0].x -gt 85 -and $_.words[-1].x -lt 950 -and $_.words[0].y -lt 590})
  if ($matches.Count -eq 1) {return $matches[0]}
  if ($matches.Count -gt 1) {throw "Ambiguous OCR target: $label"}
  if ($scroll) {Scroll $true}
 }
 throw "OCR target missing: $label"
}
function ClickLine($line,$why) { $word=$line.words[0];$last=$line.words[-1];ClickAt ([int](($word.x+$last.x+$last.w)/2)) ([int]($word.y+$word.h/2)) $why }
function Instances {ClickAt 44 114 'Instances sidebar';Start-Sleep -Seconds 1;for($i=0;$i -lt 4;$i++){Scroll $false}}
function Search {Instances;$line=Target '^search$' 'locate-search' $true;ClickLine $line 'Search other launchers';Start-Sleep -Seconds 3;for($i=0;$i -lt 4;$i++){Scroll $false}}
function FindImportButton($lines,$name,$noMods) {
 # A title is anchored to the launcher suffix; explanatory text is never a row anchor.
 $titlePattern='^'+[regex]::Escape($name)+'\s*-\s*Prism Launcher$'
 $titles=@($lines | Where-Object {$_.text -match $titlePattern -and $_.words[0].x -gt 85 -and $_.words[0].y -lt 550})
 if ($titles.Count -eq 0) {return $null}
 if ($titles.Count -ne 1) {throw 'Ambiguous fixture row title'}
 $top=($titles[0].words | ForEach-Object {[double]$_.y} | Measure-Object -Minimum).Minimum
 $next=@($lines | Where-Object {$_.text -match '^qa .+ - Prism Launcher$' -and $_.words[0].y -gt $top+8} | ForEach-Object {$_.words[0].y} | Sort-Object | Select-Object -First 1)
 # Retained 960x600 frames: minimum adjacent title pitch 74px; buttons end within 42px.
 # A 55px cap cannot reach the next observed row even if its title is missed by OCR.
 $bottom=$top+55
 if ($next.Count -eq 1) {$bottom=[Math]::Min($bottom,$next[0]-8)}
 $buttons=@(foreach($line in $lines) {
  $right=@($line.words | Where-Object {$_.x -gt 650 -and $_.x+$_.w -lt 950 -and $_.y -ge $top-3 -and $_.y+$_.h -lt [Math]::Min($bottom,590)})
  $label=($right.text -join ' ')
  # Pixel-font OCR can read the final t as E; accept only this observed substitution.
  if ($label -match '^import( withou[tE] mods)?$') {@{text=$label;words=$right}}
 })
 if ($buttons.Count -eq 0) {return $null}
 Check ('unique-import-button-'+$name) ($buttons.Count -eq 1)
 Check ('honest-import-label-'+$name) (($buttons[0].text -match ' mods$') -eq $noMods)
 return $buttons[0]
}
function ImportRow($name,$noMods) {
 $button=$null
 for ($i=0;$i -lt 6;$i++) {
  $s=Snap ('button-'+$name.Replace(' ','-'))
  $button=FindImportButton $s.lines $name $noMods
  if ($null -ne $button) {break}
  Scroll $true
 }
 if ($null -eq $button) {throw 'Fixture row button not fully visible'}
 ClickLine $button ('Import '+$name)
 Start-Sleep -Seconds 20
 Snap ('after-'+$name.Replace(' ','-'))|Out-Null
}
function VisibleCopyError($lines) {
 # Import gate prefix; ErrorLayout separately requires the complete visible tail.
 $v=OwnedWindow
 return @($lines | Where-Object {
  $words=@($_.words)
  $words.Count -ge 3 -and (($words[0..2].text -join ' ') -ieq 'Could not copy') -and
  @($words[0..2] | Where-Object {
   $_.x -le $v.r.L+85 -or $_.x+$_.w -ge $v.r.R -or
   $_.y -le $v.r.T+30 -or $_.y+$_.h -ge $v.r.B
  }).Count -eq 0
 })
}
function ResizeLauncher($width,$height) {
 $v=OwnedWindow;$client=New-Object ImportNative+RECT
 [ImportNative]::GetClientRect($v.h,[ref]$client)|Out-Null
 $dw=($v.r.R-$v.r.L)-$client.R;$dh=($v.r.B-$v.r.T)-$client.B
 [ImportNative]::SetWindowPos($v.h,[IntPtr](-1),0,0,($width+$dw),($height+$dh),0x0040)|Out-Null
 Start-Sleep -Seconds 3
 $v=OwnedWindow;[ImportNative]::GetClientRect($v.h,[ref]$client)|Out-Null
 Check ("client-size-$width-$height") ($client.R -eq $width -and $client.B -eq $height)
 $screen=[Windows.Forms.SystemInformation]::VirtualScreen
 Check ("window-in-screen-$width-$height") ($v.r.L -ge $screen.Left -and $v.r.T -ge $screen.Top -and $v.r.R -le $screen.Right -and $v.r.B -le $screen.Bottom)
}
function ErrorLayout($label) {
 # Require the complete OS-error tail, not only the prefix used by the import gate.
 $seenPrefix=$false;$seenTail=$false;$searchBottom=$null
 for($attempt=0;$attempt -lt 5;$attempt++) {
  $s=Snap $label;$v=OwnedWindow
  $prefix=@(VisibleCopyError $s.lines)
  if($prefix.Count -eq 1) {
   $seenPrefix=$true;$top=$prefix[0].words[0].y
   $search=@($s.lines|Where-Object {$_.text -match '^search$' -and $_.words[0].x -gt 85 -and $_.words[0].y -lt $top})
   Check ($label+'-search-above-error') ($search.Count -eq 1)
   $searchBottom=($search[0].words|ForEach-Object {$_.y+$_.h}|Measure-Object -Maximum).Maximum
   Check ($label+'-error-below-search') ($top -gt $searchBottom)
  }
  if($seenPrefix) {
   $textLines=@($s.lines|Where-Object {$_.words[0].x -gt $v.r.L+85 -and $_.words[0].x -lt $v.r.R-5 -and $_.words[0].y -gt $v.r.T+60 -and $_.words[0].y -lt $v.r.B-5})
   $start=if($prefix.Count -eq 1){$prefix[0].words[0].y}else{60}
   $frameWords=@($textLines|Where-Object {$_.words[0].y -ge $start}|ForEach-Object {$_.words})
   Check ($label+'-frame-'+$attempt+'-horizontal-bounds') (@($frameWords|Where-Object {$_.x -le $v.r.L+85 -or $_.x+$_.w -ge $v.r.R-5}).Count -eq 0)
   # The fixture renames only the leaf folder; symlink_metadata reports file-not-found (2).
   $tail=@($textLines|Where-Object {$_.text -match '(?i)The system cannot find the file specified\s*\.?\s*\(os error\s*2\s*\)'})
   if($tail.Count -eq 1) {
    $seenTail=$true
    $bottom=($tail[0].words|ForEach-Object {$_.y+$_.h}|Measure-Object -Maximum).Maximum
    $start=if($prefix.Count -eq 1){$prefix[0].words[0].y}else{60}
    $errorWords=@($textLines|Where-Object {$_.words[0].y -ge $start -and $_.words[0].y -le $bottom}|ForEach-Object {$_.words})
    Check ($label+'-all-visible-error-words-in-bounds') (@($errorWords|Where-Object {$_.x -le $v.r.L+85 -or $_.x+$_.w -ge $v.r.R-5 -or $_.y+$_.h -ge $v.r.B-5}).Count -eq 0)
    break
   }
  }
  Scroll $true
 }
 Check ($label+'-prefix-seen') $seenPrefix
 Check ($label+'-full-os-error-tail-seen') $seenTail
}
function NoErrorLayout($label) {
 $s=Snap $label
 Check ($label+'-no-error-prefix') (@(VisibleCopyError $s.lines).Count -eq 0)
 $line=Target '^search$' ($label+'-search') $true
 Check ($label+'-search-available') ($null -ne $line)
}
function Store {return Get-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'instances.json') -Raw|ConvertFrom-Json}
function Entry($name) {return @((Store).instances.PSObject.Properties | Where-Object {$_.Value.name -eq $name})}
function VerifyImported($id,$name,$noMods) {
 $entry=Entry $name;Check ('saved-single-'+$id) ($entry.Count -eq 1);$dest=Join-Path (Join-Path $env:HORIZON_HOME 'instances') $entry[0].Name
 Check ('destination-folder-'+$id) (Test-Path -LiteralPath $dest -PathType Container)
 $src=Join-Path $prism "instances/$id/minecraft"
 foreach ($f in (HashTree $src)) {
  $skip=$f.path -eq 'accounts.json' -or $f.path -eq 'msa_token.json' -or $f.path -eq 'logs/latest.log' -or ($noMods -and $f.path.StartsWith('mods/'))
  $target=Join-Path $dest $f.path
  if ($skip) {Check ('excluded-'+$id+'-'+$f.path) (-not (Test-Path -LiteralPath $target))}
  else {Check ('copy-'+$id+'-'+$f.path) ((Test-Path -LiteralPath $target -PathType Leaf) -and (Get-FileHash $target -Algorithm SHA256).Hash.ToLower() -eq $f.sha256)}
 }
 Check ('icon-'+$id) ((Get-FileHash (Join-Path $dest 'icon.png') -Algorithm SHA256).Hash -eq (Get-FileHash (Join-Path $prism "instances/$id/icon.png") -Algorithm SHA256).Hash)
 Check ('version-'+$id) ($entry[0].Value.java.minecraft -eq '1.21.1')
 Check ('notes-'+$id) ($entry[0].Value.notes -eq "qa fixture note`nsecond line")
 if ($noMods) {Check ('loader-absent-'+$id) ($null -eq $entry[0].Value.java.loader)} else {Check ('loader-'+$id) ($entry[0].Value.java.loader.kind -eq 'fabric' -and $entry[0].Value.java.loader.version -eq '0.16.9')}
 Json (HashTree $dest) (Join-Path $evidence ($id+'-destination.json'));return $dest
}
function StartLauncher {
 $bin=Join-Path $PWD 'launcher/target/release/horizon-launcher.exe'
 $script:proc=Start-Process -FilePath $bin -PassThru -RedirectStandardOutput (Join-Path $evidence ('launcher-'+$script:seq+'.stdout')) -RedirectStandardError (Join-Path $evidence ('launcher-'+$script:seq+'.stderr'))
 $script:birth=$script:proc.StartTime.ToUniversalTime().Ticks
 $ready=$false;for($i=0;$i -lt 120;$i++){try {$v=OwnedWindow;$ready=$true;break}catch{Start-Sleep -Milliseconds 500}}
 Check 'owned-window-ready' $ready
 $v=OwnedWindow;$client=New-Object ImportNative+RECT;[ImportNative]::GetClientRect($v.h,[ref]$client)|Out-Null
 $dw=($v.r.R-$v.r.L)-$client.R;$dh=($v.r.B-$v.r.T)-$client.B
 [ImportNative]::SetWindowPos($v.h,[IntPtr](-1),0,0,(960+$dw),(600+$dh),0x0040)|Out-Null
 Start-Sleep -Seconds 8
}
function CloseLauncher {
 if ($null -eq $script:proc) {return}
 $p=Get-Process -Id $script:proc.Id -ErrorAction SilentlyContinue
 if ($null -eq $p) {return}
 if ($p.StartTime.ToUniversalTime().Ticks -ne $script:birth) {throw 'Close birth mismatch'}
 $p.CloseMainWindow()|Out-Null
 if (-not $p.WaitForExit(12000)) {throw 'Launcher did not close normally'}
 $script:proc=$null
}
$vanished=Join-Path $prism 'instances/qa-vanished/minecraft';$held=Join-Path $prism 'instances/qa-vanished/minecraft-held'
try {
 foreach ($f in (Get-Content (Join-Path $packet 'source-manifest.json') -Raw|ConvertFrom-Json)) {Check ('packet-'+$f.path) ((Get-FileHash (Join-Path $packet $f.path) -Algorithm SHA256).Hash.ToLower() -eq $f.sha256)}
 Copy-Item -LiteralPath (Join-Path $packet 'PrismLauncher') -Destination $prism -Recurse
 $longId=('W'*100);Rename-Item -LiteralPath (Join-Path $prism 'instances/qa-vanished') -NewName $longId
 $vanished=Join-Path $prism ('instances/'+$longId+'/minecraft');$held=Join-Path $prism ('instances/'+$longId+'/minecraft-held')
 $before=HashTree $prism;Json $before (Join-Path $evidence 'source-before.json')
 $env:HORIZON_LAUNCHER_HOME=Join-Path $root 'launcher';$env:HORIZON_HOME=Join-Path $root 'game'
 foreach($p in @($env:HORIZON_LAUNCHER_HOME,$env:HORIZON_HOME)){New-Item -ItemType Directory -Force $p|Out-Null}
 @{install_new_versions=$false;skip_opening=$true;owner_notice_seen=$true;ping_notice_seen=$true;basic_summaries=$false}|ConvertTo-Json|Set-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'settings.json')
 $consentSource=Get-Content 'crates/horizon-telemetry/src/lib.rs' -Raw;$m=[regex]::Match($consentSource,'pub const CONSENT_VERSION: u32 = ([0-9]+);');if(-not $m.Success){throw 'Consent schema unknown'}
 New-Item -ItemType Directory (Join-Path $env:HORIZON_HOME 'telemetry')|Out-Null
 @{version=[int]$m.Groups[1].Value;answered=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds();choices=@{performance=$false;crashes=$false;bugs=$false;benchmarks=$false}}|ConvertTo-Json -Depth 4|Set-Content (Join-Path $env:HORIZON_HOME 'telemetry/consent.json')
 $env:HORIZON_LAUNCHER_PAGE='instances';$env:HORIZON_LAUNCHER_MOTION='off'
 $display=Get-Command Set-DisplayResolution -ErrorAction SilentlyContinue
 Check 'display-resize-command-available' ($null -ne $display)
 Set-DisplayResolution -Width 1440 -Height 900 -Force | Out-Null
 StartLauncher;ResizeLauncher 960 600;Snap 'initial-instances'|Out-Null
 Search;Snap 'discovered-rows'|Out-Null;NoErrorLayout 'no-error-960'
 ImportRow 'qa fabric import' $false;$fabric=VerifyImported 'qa-fabric' 'qa fabric import' $false
 Search;ImportRow 'qa no loader import' $true;$plain=VerifyImported 'qa-no-loader' 'qa no loader import' $true
 Search;$line=Target '^qa vanished import\s*-\s*Prism Launcher$' 'vanished-current-row' $true;Rename-Item -LiteralPath $vanished -NewName 'minecraft-held'
 ImportRow 'qa vanished import' $true
 $failureFrame=Snap 'visible-missing-source-error'
 $failure=@(VisibleCopyError $failureFrame.lines)
 Check 'missing-source-visible-error' ($failure.Count -eq 1)
 ErrorLayout 'long-unbroken-error-960'
 ResizeLauncher 1200 800;ErrorLayout 'long-unbroken-error-1200'
 ResizeLauncher 960 600
 Check 'vanished-no-metadata' ((Entry 'qa vanished import').Count -eq 0)
 Check 'vanished-no-folder' (-not (Test-Path (Join-Path $env:HORIZON_HOME 'instances/qa-vanished-import')))
 Rename-Item -LiteralPath $held -NewName 'minecraft'
 Rename-Item -LiteralPath (Join-Path $prism ('instances/'+$longId)) -NewName 'qa-vanished'
 $vanished=Join-Path $prism 'instances/qa-vanished/minecraft';$held=Join-Path $prism 'instances/qa-vanished/minecraft-held'
 Search;$line=Target '^qa vanished import\s*-\s*Prism Launcher$' 'standard-vanished-row' $true
 Rename-Item -LiteralPath $vanished -NewName 'minecraft-held'
 ImportRow 'qa vanished import' $true;ErrorLayout 'standard-error-960'
 Check 'standard-vanished-no-metadata' ((Entry 'qa vanished import').Count -eq 0)
 Check 'standard-vanished-no-folder' (-not (Test-Path (Join-Path $env:HORIZON_HOME 'instances/qa-vanished-import')))
 Rename-Item -LiteralPath $held -NewName 'minecraft'
 Rename-Item -LiteralPath (Join-Path $prism 'instances/qa-vanished') -NewName $longId
 $vanished=Join-Path $prism ('instances/'+$longId+'/minecraft');$held=Join-Path $prism ('instances/'+$longId+'/minecraft-held')
 Search;NoErrorLayout 'cleared-error-960'
 ResizeLauncher 1200 800;NoErrorLayout 'no-error-1200'
 ResizeLauncher 960 600
 $storeBefore=Get-Content (Join-Path $env:HORIZON_LAUNCHER_HOME 'instances.json') -Raw;Json (Store) (Join-Path $evidence 'instances-before-restart.json')
 CloseLauncher;StartLauncher;Snap 'reopened-instances'|Out-Null
 $null=VerifyImported 'qa-fabric' 'qa fabric import' $false;$null=VerifyImported 'qa-no-loader' 'qa no loader import' $true
 Json (Store) (Join-Path $evidence 'instances-after-restart.json')
 CloseLauncher
 $after=HashTree $prism;Json $after (Join-Path $evidence 'source-after.json')
 Check 'source-complete-tree-unchanged' ((ConvertTo-Json -InputObject $before -Compress) -ceq (ConvertTo-Json -InputObject $after -Compress))
} catch {
 if (Test-Path $prism) {Json (HashTree $prism) (Join-Path $evidence 'source-at-failure.json')}
 $checks.Add(@{name='run-exception';pass=$false;detail=$_.Exception.Message})
 try {Snap 'failure-state'|Out-Null}catch{}
} finally {
 if ((Test-Path -LiteralPath $held) -and -not (Test-Path -LiteralPath $vanished)) {Rename-Item -LiteralPath $held -NewName 'minecraft'}
 try {CloseLauncher}catch{$checks.Add(@{name='close-cleanup';pass=$false;detail=$_.Exception.Message})}
 Json $actions (Join-Path $evidence 'actions.json');Json $checks (Join-Path $evidence 'checks.json')
 Json @{source=$source;harness=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;binary_sha256=(Get-FileHash 'launcher/target/release/horizon-launcher.exe' -Algorithm SHA256).Hash;roaming=$roaming;prism=$prism;launcher_home=$env:HORIZON_LAUNCHER_HOME;game_home=$env:HORIZON_HOME;os=$env:ImageOS;image_version=$env:ImageVersion;frames=$frames;checks=$checks;limits='synthetic Prism slice only; no Play/sign-in/junction/race/mod compatibility claim'} (Join-Path $evidence 'receipt.json')
}
if (@($checks|Where-Object {-not $_.pass}).Count) {throw 'GUI qualification incomplete; retained evidence includes failure'}
