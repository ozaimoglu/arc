# Arc compatibility patch build; Arc's script is MIT, the patch and patched tool are GPL-3.0-or-later.
param(
    [string]$DotNet = 'dotnet',
    [string]$PortableArchive,
    [switch]$Install
)
$ErrorActionPreference = 'Stop'
$base = '7d8d6d362852c3427007ea81b06b6901978bef1b'
$version = '1.2.3-arc.1'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$work = Join-Path $repo ('.tools/scskiller-build-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
function Run([string]$exe, [string[]]$arguments) {
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { throw "$exe failed with exit code $LASTEXITCODE" }
}
$sdk = & $DotNet --version
if ($LASTEXITCODE -ne 0 -or [int]($sdk.Split('.')[0]) -lt 10) { throw 'Install a .NET 10 SDK or pass its executable with -DotNet.' }
$source = Join-Path $work 'source'
Run git @('clone', '--no-checkout', 'https://github.com/BlueHeisenberg/SCSKiller.git', $source)
Run git @('-C', $source, 'config', 'core.autocrlf', 'false')
Run git @('-C', $source, 'checkout', '--detach', $base)
Run git @('-C', $source, 'apply', '--check', (Join-Path $PSScriptRoot 'compat.patch'))
Run git @('-C', $source, 'apply', (Join-Path $PSScriptRoot 'compat.patch'))
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$out = Join-Path $work 'current'
$cli = Join-Path $out 'cli'
Run $DotNet @('test', (Join-Path $source 'tests/SCSKiller.Tests/SCSKiller.Tests.csproj'), '-c', 'Release', '--filter', 'FullyQualifiedName~Carved|FullyQualifiedName~ArcUnreal|FullyQualifiedName~ManualGames|FullyQualifiedName~Anti_cheat_appearing_while_the_recorder')
Run $DotNet @('publish', (Join-Path $source 'src/SCSKiller.Cli/SCSKiller.Cli.csproj'), '-c', 'Release', '-r', 'win-x64', '--self-contained', 'true', '-o', $cli,
    "-p:Version=$version", '-p:SourceRevisionId=arc-compat-1', '-p:IncludeSourceRevisionInInformationalVersion=false')
if (-not $PortableArchive) {
    $PortableArchive = Join-Path $work 'upstream-portable.zip'
    Invoke-WebRequest 'https://github.com/BlueHeisenberg/SCSKiller/releases/download/v1.2.3/SCSKiller-1.2.3-Portable.zip' -OutFile $PortableArchive
}
if ((Get-FileHash -LiteralPath $PortableArchive -Algorithm SHA256).Hash -ne '9535F1C1FAD90A30B82383A0BC8C14EE380C1549DBCFF0ED24638DF617FB3651') { throw 'The upstream portable archive checksum does not match.' }
$assets = Join-Path $work 'upstream'
Expand-Archive -LiteralPath $PortableArchive -DestinationPath $assets
$native = @(Get-ChildItem -LiteralPath $assets -Directory -Recurse | Where-Object { $_.Name -eq 'native' -and (Test-Path (Join-Path $_.FullName 'scskiller_warm.exe')) })
if ($native.Count -ne 1) { throw 'The upstream native asset layout is unexpected.' }
Copy-Item -LiteralPath $native[0].FullName -Destination (Join-Path $out 'native') -Recurse
Copy-Item -LiteralPath (Join-Path $source 'LICENSE'), (Join-Path $source 'LICENSE-EXCEPTION.txt'), (Join-Path $source 'THIRD-PARTY-NOTICES.md') -Destination $out
if (Test-Path (Join-Path $native[0].Parent.FullName 'notices')) {
    Copy-Item -LiteralPath (Join-Path $native[0].Parent.FullName 'notices') -Destination (Join-Path $out 'notices') -Recurse
}
$runtime = ((Get-Content (Join-Path $cli 'scskiller.runtimeconfig.json') -Raw | ConvertFrom-Json).runtimeOptions.includedFrameworks | Where-Object name -eq 'Microsoft.NETCore.App').version
$nuget = if ($env:NUGET_PACKAGES) { $env:NUGET_PACKAGES } else { Join-Path $env:USERPROFILE '.nuget/packages' }
$runtimeNotice = Join-Path $nuget "microsoft.netcore.app.runtime.win-x64/$runtime/THIRD-PARTY-NOTICES.TXT"
if (Test-Path -LiteralPath $runtimeNotice) {
    New-Item -ItemType Directory -Force (Join-Path $out 'notices') | Out-Null
    Copy-Item -LiteralPath $runtimeNotice -Destination (Join-Path $out 'notices/dotnet-THIRD-PARTY-NOTICES.txt')
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'compat.patch') -Destination $out
$manifest = @{ Protocol = 1; Version = $version; BaseCommit = $base }
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $cli 'arc-compat.json') -Encoding utf8
Run (Join-Path $cli 'scskiller.exe') @('arc-info')
if ($Install) {
    $destination = Join-Path $env:LOCALAPPDATA "Programs/ArcTools/SCSKiller-$version"
    if (Test-Path -LiteralPath $destination) { throw "An installation already exists at $destination. Keep it or choose a new compatibility version." }
    New-Item -ItemType Directory -Path $destination | Out-Null
    Copy-Item -LiteralPath $out -Destination (Join-Path $destination 'current') -Recurse
    # Retain the complete patched source beside the local installation, without its Git object database.
    Run git @('-C', $source, 'archive', '--format=zip', '--output', (Join-Path $destination 'upstream-source.zip'), $base)
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'compat.patch'), (Join-Path $PSScriptRoot 'README.md'), $PSCommandPath -Destination $destination
    Write-Host "Connect this CLI in Arc Settings: $(Join-Path $destination 'current/cli/scskiller.exe')"
} else { Write-Host "Compatibility build: $out" }
Write-Host 'Build workspace retained for inspection. This script does not modify game folders, cache limits or scheduled tasks.'
