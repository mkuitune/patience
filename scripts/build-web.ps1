param([switch]$Dev)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $buildProfile = if ($Dev) { '--dev' } else { '--release' }
    & wasm-pack build apps/desktop --target web --out-dir ../../dist/engines/v1 --out-name patience_app $buildProfile --no-pack
    if ($LASTEXITCODE -ne 0) { throw 'WebAssembly build failed.' }
    $assetVersion = (Get-FileHash -LiteralPath dist/engines/v1/patience_app_bg.wasm -Algorithm SHA256).Hash.ToLowerInvariant()
    $html = (Get-Content -LiteralPath web/index.html -Raw -Encoding UTF8).Replace('BUILD_ASSET_VERSION', $assetVersion)
    Set-Content -LiteralPath dist/index.html -Value $html -Encoding UTF8
    Write-Host 'Web build ready in dist. Serve that directory over HTTP.'
} finally {
    Pop-Location
}
