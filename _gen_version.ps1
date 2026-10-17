param(
    [Parameter(Mandatory=$true)][string]$LabVersion,
    [Parameter(Mandatory=$true)][string]$VersionFile
)

$existing = Get-Content -LiteralPath $VersionFile -Raw -Encoding UTF8
$kitMatch = [regex]::Match($existing, 'public static final String VERSION = "([^"]+)";')
if (-not $kitMatch.Success) { throw '找不到现有Kit版本；禁止用Lab版本覆盖Kit版本。' }
$kitVersion = $kitMatch.Groups[1].Value
$content = @"
package org.example;

/** Kit与Lab独立维护版本；生成脚本保留现有Kit版本。 */
public final class Version {
    public static final String VERSION = "$kitVersion";
    public static final String LAB_VERSION = "$LabVersion";
    private Version() {}
}
"@

$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText((Resolve-Path $VersionFile).Path, $content, $utf8NoBom)
Write-Host "版本已生成：Lab=$LabVersion，Kit=$kitVersion（保留）"
