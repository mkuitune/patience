param([switch]$Dev)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $buildProfile = if ($Dev) { '--dev' } else { '--release' }
    $html = Get-Content -LiteralPath web/index.html -Raw -Encoding UTF8
    foreach ($backend in @('webgpu', 'webgl2')) {
        & wasm-pack build apps/desktop --target web --out-dir "../../dist/engines/v1/$backend" --out-name patience_app $buildProfile --no-pack --no-default-features --features $backend
        if ($LASTEXITCODE -ne 0) { throw "WebAssembly build failed for $backend." }
        $assetVersion = (Get-FileHash -LiteralPath "dist/engines/v1/$backend/patience_app_bg.wasm" -Algorithm SHA256).Hash.ToLowerInvariant()
        $html = $html.Replace("BUILD_ASSET_VERSION_$($backend.ToUpperInvariant())", $assetVersion)
    }
    Set-Content -LiteralPath dist/index.html -Value $html -Encoding UTF8
    Set-Content -LiteralPath dist/card-preview.html -Value $html -Encoding UTF8
    Write-Host 'Web build ready in dist. Serve that directory over HTTP.'
} finally {
    Pop-Location
}
