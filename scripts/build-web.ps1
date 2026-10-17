param([switch]$Dev)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $buildProfile = if ($Dev) { '--dev' } else { '--release' }
    & wasm-pack build apps/desktop --target web --out-dir ../../dist/engines/v1 --out-name patience_app $buildProfile --no-pack
    if ($LASTEXITCODE -ne 0) { throw 'WebAssembly build failed.' }
    Copy-Item -LiteralPath web/index.html -Destination dist/index.html
    Write-Host 'Web build ready in dist. Serve that directory over HTTP.'
} finally {
    Pop-Location
}
