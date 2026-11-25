$ErrorActionPreference='Stop'
# Shared run-wide base64 budget is script scope, initialized once by workflow.
if ($null -eq $script:ClientEncodedTotal) { throw 'Missing shared export budget' }
$maxPngBytes=4000000;$maxEncodedTotal=24000000
$info=Join-Path $env:CAP 'window-info.txt'
if (Test-Path -LiteralPath $info) {
  $fi=Get-Item -LiteralPath $info
  if ($fi.Length -gt 65536) { Write-Output 'INFO-SKIP: exceeds 65536 bytes' }
  else {
    $lines=@(Get-Content -LiteralPath $info -TotalCount 121)
    if ($lines.Count -gt 120) { Write-Output 'INFO-TRUNCATED: 120-line ceiling' }
    foreach ($line in ($lines | Select-Object -First 120)) {
      $safe=($line -replace '[^\x20-\x7E]','?')
      if ($safe.Length -gt 500) {$safe=$safe.Substring(0,500)}
      Write-Output $safe
      # Summary is plain fenced text, strips markup to avoid untrusted rendering.
      $summary=$safe -replace '[`<>|]','_'
      if ($script:ClientSummaryLines -lt 720) { Add-Content $env:GITHUB_STEP_SUMMARY $summary; $script:ClientSummaryLines++ }
    }
  }
} else { Write-Output 'NO-EVIDENCE: bounded info file absent' }
$expected=@('frame-01-top.png','frame-02-repeat.png')
$found=0
foreach ($name in $expected) {
  $path=Join-Path $env:CAP $name
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Write-Output "FRAME-MISSING: $name";continue }
  $png=Get-Item -LiteralPath $path; $enc=4*[Math]::Ceiling($png.Length/3)
  if ($png.Length -eq 0 -or $png.Length -gt $maxPngBytes -or $script:ClientEncodedTotal+$enc -gt $maxEncodedTotal) { Write-Output "FRAME-SKIP: $name size-or-shared-budget";continue }
  $bytes=[IO.File]::ReadAllBytes($path);$sha=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
  if ($bytes.Length -ne $png.Length) { throw 'Frame changed during read' }
  Write-Output "###FRAME-BEGIN desktop-$env:CAP_ROUTE-$name sha256=$sha bytes=$($bytes.Length)"
  $b64=[Convert]::ToBase64String($bytes)
  for ($i=0;$i -lt $b64.Length;$i+=76) {Write-Output $b64.Substring($i,[Math]::Min(76,$b64.Length-$i))}
  Write-Output "###FRAME-END desktop-$env:CAP_ROUTE-$name"
  $script:ClientEncodedTotal+=$enc;$found++
}
Write-Output "###EXPORT-SUMMARY frames=$found expected=2 shared-encoded=$script:ClientEncodedTotal ceiling=$maxEncodedTotal"
if ($found -ne 2) { throw 'Incomplete bounded frame export' }
