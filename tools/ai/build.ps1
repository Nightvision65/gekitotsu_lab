param(
    [string]$Maven = 'mvn',
    [string]$Cargo = 'cargo',
    [int]$TimeoutSeconds = 300,
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$taskRepo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Push-Location $taskRepo
try {
    if ($env:CONDA_DEFAULT_ENV -ne 'opencode') { throw '请先conda activate opencode' }
    $env:PYTHONIOENCODING = 'utf-8'
    # Maven国内镜像独立执行，Rust境外下载仅对子进程设置代理。
    $env:GKT_MAVEN = $Maven
    $env:GKT_CARGO = $Cargo
    $env:GKT_BUILD_TIMEOUT = "$TimeoutSeconds"
    $env:GKT_OFFLINE = if ($Offline) { '1' } else { '0' }
    python tools/ai/build.py
    if ($LASTEXITCODE -ne 0) { throw 'AI工程构建失败' }
} finally {
    foreach ($taskKey in @('GKT_MAVEN','GKT_CARGO','GKT_BUILD_TIMEOUT','GKT_OFFLINE')) {
        Remove-Item -LiteralPath "Env:$taskKey" -ErrorAction SilentlyContinue
    }
    Pop-Location
}
