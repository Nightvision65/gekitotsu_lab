param([Parameter(ValueFromRemainingArguments = $true)][string[]]$WorkflowArguments)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$env:PYTHONIOENCODING = 'utf-8'
if ($env:CONDA_DEFAULT_ENV -ne 'opencode') {
    if (-not (Get-Command conda -ErrorAction SilentlyContinue)) { throw '找不到conda，请先安装并初始化PowerShell。' }
    (& conda 'shell.powershell' 'hook') | Out-String | Invoke-Expression
    conda activate opencode
}
if ($env:CONDA_DEFAULT_ENV -ne 'opencode') { throw '未能激活conda opencode。' }
if (-not $WorkflowArguments) { $WorkflowArguments = @('--help') }
& python (Join-Path $PSScriptRoot 'formation_workflow.py') @WorkflowArguments
exit $LASTEXITCODE
