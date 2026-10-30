# Horizon Studio's MCP server for AI agents, in one line (PowerShell):
#   irm https://raw.githubusercontent.com/vermingov/horizon-studio/main/mcp/install.ps1 | iex
# Downloads horizon-studio-mcp from the latest release, checks it against the
# release's SHA256SUMS, puts it in %LOCALAPPDATA%\horizon-studio\bin and
# registers it with the agents installed here. Run it again to update.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = if ($env:HORIZON_STUDIO_REPO) { $env:HORIZON_STUDIO_REPO } else { 'vermingov/horizon-studio' }
$asset = 'horizon-studio-mcp-x86_64-pc-windows-msvc.zip'
$base = "https://github.com/$repo/releases/latest/download"
$dir = Join-Path $env:LOCALAPPDATA 'horizon-studio\bin'
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $dir, $tmp | Out-Null
try {
    Write-Host "Downloading $asset ..."
    Invoke-WebRequest "$base/$asset" -OutFile (Join-Path $tmp $asset) -UseBasicParsing
    Invoke-WebRequest "$base/SHA256SUMS" -OutFile (Join-Path $tmp 'SHA256SUMS') -UseBasicParsing
    $want = Get-Content (Join-Path $tmp 'SHA256SUMS') |
        ForEach-Object { $p = $_ -split '\s+'; if ($p[1] -eq $asset) { $p[0] } } |
        Select-Object -First 1
    $got = (Get-FileHash (Join-Path $tmp $asset) -Algorithm SHA256).Hash.ToLower()
    if (-not $want -or $want -ne $got) { throw "The download doesn't match the release's checksum: not installing it." }
    Expand-Archive (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    $exe = Join-Path $dir 'horizon-studio-mcp.exe'
    Copy-Item (Join-Path $tmp 'horizon-studio-mcp.exe') $exe -Force
    Write-Host "Installed $(& $exe --version) in $dir"
    Write-Host ''
    & $exe install
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
