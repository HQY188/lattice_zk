param(
  [string]$DataDir = "",
  [switch]$NoCapture,
  [switch]$IncludeEmpty
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
  if ($PSScriptRoot) {
    return (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
  }
  return (Resolve-Path (Join-Path (Get-Location).Path "..")).Path
}

$repoRoot = Get-RepoRoot
if ([string]::IsNullOrWhiteSpace($DataDir)) {
  $DataDir = Join-Path $repoRoot "data"
}
$DataDir = (Resolve-Path $DataDir).Path

if (!(Test-Path $DataDir)) {
  throw "data directory not found: $DataDir"
}

# Libra(raw) correctness test (Raw PCS + SHA256) over all datasets under data/.
# We iterate all '*circuit*.txt' datasets that have a matching witness.
$circuits = Get-ChildItem -LiteralPath $DataDir -File -Filter "*circuit*.txt" | Sort-Object Name
if ($circuits.Count -eq 0) {
  throw "no circuit files found under: $DataDir (pattern: *circuit*.txt)"
}

$args = @("test", "-p", "gkr", "--release", "gkr_correctness_libra_case")
if ($NoCapture) { $args += @("--", "--nocapture") }
elseif (-not $NoCapture.IsPresent) { $args += @("--", "--nocapture") }

Write-Host ("[libra] dataDir = {0}" -f $DataDir)
Write-Host ("[libra] cases  = {0}" -f $circuits.Count)

$ok = 0
$skippedEmpty = 0
$skippedUnknown = 0
foreach ($c in $circuits) {
  $witnessName = $c.Name -replace "circuit", "witness"
  $wPath = Join-Path $DataDir $witnessName
  if (!(Test-Path $wPath)) {
    Write-Warning ("[libra] skip (no witness): {0} -> {1}" -f $c.Name, $witnessName)
    continue
  }
  $w = Get-Item -LiteralPath $wPath
  if (-not $IncludeEmpty.IsPresent -and ($c.Length -eq 0 -or $w.Length -eq 0)) {
    Write-Warning ("[libra] skip (empty file): {0} ({1} bytes), {2} ({3} bytes)" -f $c.Name, $c.Length, $w.Name, $w.Length)
    $skippedEmpty++
    continue
  }

  $lower = $c.Name.ToLowerInvariant()
  $field = ""
  if ($lower -match "bn254") { $field = "bn254" }
  elseif ($lower -match "babybear") { $field = "babybear" }
  elseif ($lower -match "goldilocks") { $field = "goldilocks" }
  elseif ($lower -match "gf2") { $field = "gf2" }
  elseif ($lower -match "m31") { $field = "m31" }

  if ([string]::IsNullOrWhiteSpace($field)) {
    Write-Warning ("[libra] skip (unknown field): {0}" -f $c.Name)
    $skippedUnknown++
    continue
  }

  $env:GKR_TEST_CIRCUIT_PATH = $c.FullName
  $env:GKR_TEST_WITNESS_PATH = (Resolve-Path $wPath).Path
  $env:GKR_TEST_FIELD_TYPE = $field

  Write-Host ("`n[libra] run({0}): {1} / {2}" -f $field, $c.Name, (Split-Path -Leaf $wPath))
  & cargo @args
  if ($LASTEXITCODE -ne 0) {
    throw ("[libra] FAILED: {0}" -f $c.Name)
  }
  $ok++
}

Write-Host ("`n[libra] DONE: {0} case(s) passed; {1} empty skipped; {2} unknown skipped" -f $ok, $skippedEmpty, $skippedUnknown)

