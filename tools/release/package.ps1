param(
    [string]$Maven = 'mvn',
    [string]$Cargo = 'cargo',
    [string]$Output = 'dist',
    [int]$TimeoutSeconds = 300,
    [switch]$Offline,
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
if ($env:CONDA_DEFAULT_ENV -ne 'opencode') { throw '请先conda activate opencode。' }
$releaseRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
Push-Location $releaseRoot
try {
    $env:PYTHONIOENCODING = 'utf-8'
    $releaseArguments = @('--maven', $Maven, '--cargo', $Cargo, '--out', $Output, '--timeout', "$TimeoutSeconds")
    if ($Offline) { $releaseArguments += '--offline' }
    if ($SkipBuild) { $releaseArguments += '--skip-build' }
    & python (Join-Path $PSScriptRoot 'package.py') @releaseArguments
    if ($LASTEXITCODE -ne 0) { throw '发布构建失败；检查输出日志。' }
} finally { Pop-Location }
