$ErrorActionPreference = 'Continue'
# Export ceilings, checked BEFORE reading each file: no unbounded
# ReadAllBytes/base64 into the log. Per-file raw limit 1,500,000 bytes;
# total ENCODED base64 characters across all frames 6,000,000.
$maxPngBytes = 4000000
$maxEncodedTotal = 24000000
$encodedTotal = 0
$info = "$env:CAP\window-info.txt"
if (Test-Path $info) {
  Get-Content $info | ForEach-Object { Write-Output $_ }
  Get-Content $info | ForEach-Object { Add-Content $env:GITHUB_STEP_SUMMARY "| $_ |" }
  Add-Content $env:GITHUB_STEP_SUMMARY "`nFrames are base64 blocks in this run's raw log between ###FRAME-BEGIN and ###FRAME-END markers. Decode: copy a block, then ``base64 -d > frame.png``."
} else {
  Write-Output "NO-EVIDENCE: window-info.txt missing (capture step died before writing)"
  Add-Content $env:GITHUB_STEP_SUMMARY "NO-EVIDENCE: capture step produced no window-info - see job log."
}
$frames = @(Get-ChildItem "$env:CAP\frame-*.png" -ErrorAction SilentlyContinue)
if ($frames.Count -eq 0) { Write-Output "###FRAMES-NONE found=0 expected<=3 - no frame files present" }
elseif ($frames.Count -lt 3) { Write-Output "###FRAMES-INCOMPLETE found=$($frames.Count) expected=3" }
foreach ($png in $frames) {
  $len = $png.Length
  $enc = 4 * [Math]::Ceiling($len / 3)
  if ($len -gt $maxPngBytes) {
    Write-Output "###FRAME-SKIP $env:CAP_KIND-$env:CAP_ROUTE-$($png.Name) reason=oversize bytes=$len limit=$maxPngBytes"
    continue
  }
  if ($encodedTotal + $enc -gt $maxEncodedTotal) {
    Write-Output "###FRAME-SKIP $env:CAP_KIND-$env:CAP_ROUTE-$($png.Name) reason=total-ceiling encoded-would-be=$($encodedTotal + $enc) limit=$maxEncodedTotal"
    continue
  }
  $bytes = [IO.File]::ReadAllBytes($png.FullName)
  $sha = (Get-FileHash $png.FullName -Algorithm SHA256).Hash
  Write-Output "###FRAME-BEGIN $env:CAP_KIND-$env:CAP_ROUTE-$($png.Name) sha256=$sha bytes=$($bytes.Length)"
  $b64 = [Convert]::ToBase64String($bytes)
  for ($i = 0; $i -lt $b64.Length; $i += 76) {
    Write-Output $b64.Substring($i, [Math]::Min(76, $b64.Length - $i))
  }
  Write-Output "###FRAME-END $env:CAP_KIND-$env:CAP_ROUTE-$($png.Name)"
  $encodedTotal += $enc
}
Write-Output "###EXPORT-SUMMARY frames-present=$($frames.Count) encoded-chars=$encodedTotal ceiling=$maxEncodedTotal"
