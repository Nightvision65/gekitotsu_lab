param(
    [string]$Cargo = 'cargo',
    [int]$TimeoutSeconds = 300,
    [switch]$Benchmark
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
if ($env:CONDA_DEFAULT_ENV -ne 'opencode') { throw '请先conda activate opencode' }
$taskRepo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Push-Location $taskRepo
try {
    $env:PYTHONIOENCODING = 'utf-8'
    $env:GKT_CARGO = $Cargo
    $env:GKT_TEST_TIMEOUT = "$TimeoutSeconds"
    $env:GKT_BENCHMARK = if ($Benchmark) { '1' } else { '0' }
    python tools/ai/test.py
    if ($LASTEXITCODE -ne 0) { throw 'AI验收失败' }
} finally {
    foreach ($taskKey in @('GKT_CARGO','GKT_TEST_TIMEOUT','GKT_BENCHMARK')) {
        Remove-Item -LiteralPath "Env:$taskKey" -ErrorAction SilentlyContinue
    }
    Pop-Location
}
