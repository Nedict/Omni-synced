$ErrorActionPreference = 'Continue'
Set-Location 'C:\Users\willi\source\solidcraft'
$bad = 'https://devops.quasarke.net/wdunn001/solidcraft-cam-libraries.git'
$branches = @(
  'feat/companion-cam-ui','feat/freecad-cam-adapter','feat/manufacture-setup','feat/mfg-document-core',
  'feat/mfg-library-sync','feat/mfg-machines-presets','feat/mfg-plugin-hosts','feat/mfg-post-grbl',
  'feat/mfg-probe-snippets','feat/mfg-protocol-focas','feat/mfg-protocol-mtconnect','feat/mfg-protocol-thinc',
  'feat/mfg-setup-wcs','feat/mfg-simulate','feat/mfg-ui-2d','feat/mfg-ui-3d','feat/mfg-ui-additive',
  'feat/mfg-ui-contour','feat/mfg-ui-drill','feat/mfg-ui-fabrication','feat/mfg-ui-hole_recognition',
  'feat/mfg-ui-inspection','feat/mfg-ui-libraries','feat/mfg-ui-modify','feat/mfg-ui-multiaxis',
  'feat/mfg-ui-part_handling','feat/mfg-ui-post','feat/mfg-ui-tools','feat/mfg-ui-turning',
  'feat/mfg-workspace-shell','feat/solidcraft-cam-app'
)
$fixed = New-Object System.Collections.Generic.List[string]
$failed = New-Object System.Collections.Generic.List[string]

foreach ($b in $branches) {
  git checkout -B $b "fork/$b" 2>$null | Out-Null
  if ($LASTEXITCODE -ne 0) {
    git fetch fork $b 2>$null | Out-Null
    git checkout -B $b "fork/$b" 2>$null | Out-Null
  }
  $hits = @(git grep -l -- $bad 2>$null)
  if ($hits.Count -eq 0) {
    Write-Host "skip $b (already clean)"
    continue
  }
  foreach ($f in $hits) {
    $path = Join-Path (Get-Location) $f
    $raw = [IO.File]::ReadAllText($path)
    $raw2 = $raw.Replace($bad, '')
    if ($raw2 -ne $raw) {
      [IO.File]::WriteAllText($path, $raw2)
    }
  }

  $eng = 'crates/engine/src/cmd/manufacture.rs'
  if (Test-Path $eng) {
    $path = (Resolve-Path $eng).Path
    $e = [IO.File]::ReadAllText($path)
    if ($e -match 'library_ensure_default_remote' -and $e -notmatch 'DEFAULT_CAM_LIBRARY_REMOTE\.trim\(\)\.is_empty') {
      $guard = @'
    if library::DEFAULT_CAM_LIBRARY_REMOTE.trim().is_empty() {
        return Ok(json!({"added": false, "remotes": mfg.library_remotes}));
    }
'@
      $marker = 'let mut mfg = s.doc.manufacture.clone();'
      $idx = $e.IndexOf($marker)
      if ($idx -ge 0) {
        $insertAt = $idx + $marker.Length
        # find this occurrence inside ensure_default by searching nearby
        $ensureIdx = $e.IndexOf('fn library_ensure_default_remote')
        if ($ensureIdx -ge 0) {
          $idx = $e.IndexOf($marker, $ensureIdx)
          if ($idx -ge 0) {
            $insertAt = $idx + $marker.Length
            $nl = if ($e.Contains("`r`n")) { "`r`n" } else { "`n" }
            $e = $e.Insert($insertAt, $nl + $guard.TrimEnd() + $nl)
            [IO.File]::WriteAllText($path, $e)
          }
        }
      }
    }
  }

  git add -A
  if (git diff --cached --quiet) {
    Write-Host "skip $b (no staged)"
    continue
  }
  git commit -m 'Remove private lab hostname from default CAM library remote.' | Out-Null
  git push --force-with-lease fork ("HEAD:refs/heads/" + $b) 2>&1 | Out-Null
  if ($LASTEXITCODE -eq 0) {
    $fixed.Add($b) | Out-Null
    Write-Host "OK $b"
  } else {
    $failed.Add($b) | Out-Null
    Write-Host "FAIL $b"
  }
}

Write-Host ("==== fixed {0} failed {1} ====" -f $fixed.Count, $failed.Count)
$failed | ForEach-Object { Write-Host "FAIL $_" }

git fetch fork 2>$null | Out-Null
$still = @()
foreach ($b in $branches) {
  git grep -q -- $bad ("fork/" + $b) 2>$null
  if ($LASTEXITCODE -eq 0) { $still += $b }
}
Write-Host ("==== still dirty tips: {0} ====" -f $still.Count)
$still | ForEach-Object { Write-Host "DIRTY $_" }
git checkout integrate/mfg-fork-main 2>$null | Out-Null
