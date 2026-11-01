# Installs or updates Horizon Studio in Horizon's launcher on Windows, in seconds where it can.
#
#   irm https://raw.githubusercontent.com/vermingov/horizon-studio/main/install.ps1 | iex
#
# install.sh's Windows twin, step for step. Horizon Studio is a native mod: the launcher compiles
# it into the game (10 to 30 minutes). Each release also carries that game already built for
# Windows (x64), built the way Horizon builds its own Windows game. This script puts it in the
# launcher's versions folder under the name the launcher gives that build, then runs the
# launcher's own `native-build`, which finds it there and only selects it for an instance. The
# bundle goes to the launcher as its registry ships mods (horizon-native.json beside
# mixins\<id>\), pinning the horizon-mixins commit the game was built with: launchers name a
# build after the pinned commit, where a bare bundle would get the launcher's own, which changes
# with its updates. Where there is no prebuilt game, the launcher compiles as before, with
# settings that compile faster.
#
# Close the launcher first. Environment:
#   HORIZON_LAUNCHER       the launcher program (default: horizon-launcher.exe on the PATH, else
#                          in %LOCALAPPDATA%\Horizon or %LOCALAPPDATA%\Programs\Horizon)
#   HORIZON_LAUNCHER_HOME  the launcher's data folder, as the launcher reads it
#                          (default: %APPDATA%\horizon-launcher)
#   HORIZON_STUDIO_BUILD=1 compile instead of using the prebuilt game
#
# For tests: `install.ps1 --key <bundle> <mixins> <profile> <features> <rust> <revision>` prints
# the key install.sh's `--key` prints for the same inputs (tools/studio.py native-key checks both).
& {
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$REPO = 'vermingov/horizon-studio'
$ID = 'horizon_studio'

function Say([string]$Text) { Write-Host $Text }

# The launcher's version_for (launcher/src/native_mods.rs) for one flat bundle folder, as
# install.sh's key_of computes it: native-<7 digits of the revision>-<12 hex digits of SHA-256
# over the inputs, each followed by a NUL, then the mod's id and a NUL, then each file (by name,
# in byte order, skipping names that start with a dot as `ls` does) as name, length as 8
# little-endian bytes, bytes>.
function Get-Key([string]$Bundle, [string]$Mixins, [string]$BuildProfile, [string]$Features, [string]$Rust, [string]$Revision) {
    $utf8 = New-Object System.Text.UTF8Encoding $false
    $buf = New-Object System.IO.MemoryStream
    foreach ($part in @($Mixins, $BuildProfile, $Features, $Rust, $Revision, $ID)) {
        $bytes = $utf8.GetBytes($part)
        $buf.Write($bytes, 0, $bytes.Length)
        $buf.WriteByte(0)
    }
    $names = [string[]]@(Get-ChildItem -LiteralPath $Bundle -File -Force |
        Where-Object { -not $_.Name.StartsWith('.', [StringComparison]::Ordinal) } |
        ForEach-Object { $_.Name })
    [Array]::Sort($names, [StringComparer]::Ordinal)
    foreach ($name in $names) {
        $bytes = $utf8.GetBytes($name)
        $buf.Write($bytes, 0, $bytes.Length)
        $data = [IO.File]::ReadAllBytes([IO.Path]::Combine($Bundle, $name))
        $length = [BitConverter]::GetBytes([UInt64]$data.LongLength)
        if (-not [BitConverter]::IsLittleEndian) { [Array]::Reverse($length) }
        $buf.Write($length, 0, 8)
        $buf.Write($data, 0, $data.Length)
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    $hex = [BitConverter]::ToString($sha.ComputeHash($buf.ToArray())).Replace('-', '').ToLowerInvariant()
    $short = if ($Revision.Length -gt 7) { $Revision.Substring(0, 7) } else { $Revision }
    "native-$short-$($hex.Substring(0, 12))"
}

# For tests: the key of a bundle, to compare with tools/studio.py native-key. (PowerShell may
# hand the flag over as --key, -key or -key:, so all are read as it.)
if ($args.Count -ge 1 -and "$($args[0])".TrimStart('-').TrimEnd(':') -eq 'key') {
    Get-Key $args[1] $args[2] $args[3] $args[4] $args[5] $args[6]
    return
}

# One argument as Windows programs (Rust's std::env::args) split a command line.
function ConvertTo-Argument([string]$Text) {
    if ($Text -ne '' -and $Text -notmatch '[\s"]') { return $Text }
    $out = '"'
    $slashes = 0
    foreach ($c in $Text.ToCharArray()) {
        if ($c -eq [char]'\') { $slashes++; continue }
        if ($c -eq [char]'"') { $out += ('\' * (2 * $slashes + 1)) + '"' }
        else { $out += ('\' * $slashes) + $c }
        $slashes = 0
    }
    $out + ('\' * (2 * $slashes)) + '"'
}

# The launcher, waited for: it is a windowed program, which PowerShell would not wait for (and
# whose exit code it would not keep) if it were run directly. Returns its exit code and standard
# output; its errors and progress go to this console (a redirected output also hands the
# launcher this console's error handle, which it keeps).
function Invoke-Launcher([string[]]$Arguments, [hashtable]$Environment = @{}) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $launcher
    $psi.Arguments = (@($Arguments | ForEach-Object { ConvertTo-Argument $_ })) -join ' '
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.StandardOutputEncoding = New-Object System.Text.UTF8Encoding $false
    foreach ($name in $Environment.Keys) { $psi.EnvironmentVariables[$name] = $Environment[$name] }
    $p = [System.Diagnostics.Process]::Start($psi)
    $out = $p.StandardOutput.ReadToEnd()
    $p.WaitForExit()
    [pscustomobject]@{ Code = $p.ExitCode; Out = "$out".Trim() }
}

# A text file as the launcher reads it: UTF-8 without a byte order mark.
function Write-Text([string]$Path, [string]$Text) {
    [IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding $false))
}

function Remove-Folder([string]$Path) {
    if (Test-Path -LiteralPath $Path) { Remove-Item -LiteralPath $Path -Recurse -Force }
}

# ----- The launcher, its data folder, the platform -----

try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch { }

# The launcher comes as a zip that people unpack wherever they like (and it updates itself in
# place), so it is looked for: HORIZON_LAUNCHER, the PATH, the launcher running now, the
# shortcuts to it (the Desktop's and the Start menu's), then the folders people unpack into, a
# few levels down. Failing those, the person is asked where it is.
function Test-Launcher([string]$path) {
    if (-not $path) { return $null }
    $path = $path.Trim().Trim('"')
    if (Test-Path -LiteralPath $path -PathType Container) {
        $inside = Join-Path $path 'horizon-launcher.exe'
        if (Test-Path -LiteralPath $inside -PathType Leaf) { return $inside }
        $deeper = Get-ChildItem -LiteralPath $path -Filter 'horizon-launcher*.exe' -Recurse -Depth 3 -File -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($deeper) { return $deeper.FullName }
        return $null
    }
    if (Test-Path -LiteralPath $path -PathType Leaf) { return (Resolve-Path -LiteralPath $path).Path }
    return $null
}

function Find-Launcher {
    $found = Test-Launcher $env:HORIZON_LAUNCHER
    if ($found) { return $found }
    $onPath = Get-Command horizon-launcher -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($onPath) { return $onPath.Path }
    # The launcher open now.
    $running = Get-Process -Name 'horizon-launcher*' -ErrorAction SilentlyContinue | Where-Object { $_.Path } | Select-Object -First 1
    if ($running) { return $running.Path }
    # Shortcuts: the launcher's own (an instance's Play shortcut runs the launcher) and any the
    # person made.
    $shell = $null
    try { $shell = New-Object -ComObject WScript.Shell } catch { }
    if ($shell) {
        $places = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('CommonDesktopDirectory'),
            [Environment]::GetFolderPath('StartMenu'), [Environment]::GetFolderPath('CommonStartMenu'),
            [Environment]::GetFolderPath('Programs')) | Where-Object { $_ -and (Test-Path -LiteralPath $_) }
        foreach ($place in $places) {
            foreach ($lnk in Get-ChildItem -LiteralPath $place -Filter '*.lnk' -Recurse -Depth 2 -File -ErrorAction SilentlyContinue) {
                try {
                    $target = $shell.CreateShortcut($lnk.FullName).TargetPath
                    if ($target -and ([IO.Path]::GetFileName($target) -like 'horizon-launcher*.exe') -and (Test-Path -LiteralPath $target -PathType Leaf)) {
                        return $target
                    }
                } catch { }
            }
        }
    }
    # The folders people unpack into.
    $userProfile = $env:USERPROFILE
    $roots = @(
        [IO.Path]::Combine($env:LOCALAPPDATA, 'Horizon'),
        [IO.Path]::Combine($env:LOCALAPPDATA, 'Programs'),
        (Join-Path $userProfile 'Downloads'),
        [Environment]::GetFolderPath('Desktop'),
        [Environment]::GetFolderPath('MyDocuments'),
        $env:LOCALAPPDATA,
        $env:APPDATA,
        $env:ProgramFiles,
        ${env:ProgramFiles(x86)},
        $userProfile
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -Unique
    foreach ($root in $roots) {
        $depth = if ($root -eq $userProfile) { 2 } else { 4 }
        $hit = Get-ChildItem -LiteralPath $root -Filter 'horizon-launcher*.exe' -Recurse -Depth $depth -File -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -notmatch '\\(versions|cache|\.horizon-studio)\\' } | Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    # The roots of the drives (C:\Horizon, D:\Games\Horizon...).
    foreach ($drive in Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue) {
        $hit = Get-ChildItem -LiteralPath $drive.Root -Filter 'horizon-launcher*.exe' -Recurse -Depth 2 -File -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    # Asked, when there is someone to ask.
    if ([Environment]::UserInteractive -and -not $env:CI) {
        Say 'Could not find Horizon''s launcher (horizon-launcher.exe).'
        for ($i = 0; $i -lt 3; $i++) {
            $answer = Read-Host 'Paste the path to horizon-launcher.exe or its folder (Enter to stop)'
            if (-not $answer) { break }
            $found = Test-Launcher $answer
            if ($found) { return $found }
            Say "No horizon-launcher.exe at $answer."
        }
    }
    return $null
}

$launcher = Find-Launcher
if ($launcher) { Say "Horizon's launcher: $launcher" }

# The launcher open while this installs would hold the files it changes: asked to close first.
function Wait-LauncherClosed {
    $open = @(Get-Process -Name 'horizon-launcher*' -ErrorAction SilentlyContinue)
    if ($open.Count -eq 0) { return }
    if (-not [Environment]::UserInteractive -or $env:CI) {
        Say 'Horizon''s launcher is open: close it if the install fails.'
        return
    }
    Say 'Horizon''s launcher is open. Close it (and the game), then press Enter.'
    for ($i = 0; $i -lt 5; $i++) {
        [void](Read-Host)
        if (@(Get-Process -Name 'horizon-launcher*' -ErrorAction SilentlyContinue).Count -eq 0) { return }
        Say 'It is still open: close it from its window (or the system tray), then press Enter.'
    }
}

# The launcher's data folder: dirs::data_dir() (%APPDATA%, the roaming one) + horizon-launcher.
$data = ''
if ($env:HORIZON_LAUNCHER_HOME) { $data = $env:HORIZON_LAUNCHER_HOME }
elseif ($env:APPDATA) { $data = [IO.Path]::Combine($env:APPDATA, 'horizon-launcher') }
# PROCESSOR_ARCHITEW6432 is the machine's when this is a 32-bit PowerShell.
$arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$target = if ($arch -eq 'AMD64') { 'x86_64-pc-windows-msvc' } else { '' }

$cacheRoot = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { [IO.Path]::GetTempPath() }
$cache = [IO.Path]::Combine($cacheRoot, 'horizon-studio', 'cache')

# The installed Horizon version to build on: the newest prepared one that isn't itself a native
# build, preferring one of the game version the studio is made for.
function Get-BaseVersion([string]$Game) {
    $versions = [IO.Path]::Combine($data, 'versions')
    if (-not (Test-Path -LiteralPath $versions -PathType Container)) { return '' }
    $names = [string[]]@(Get-ChildItem -LiteralPath $versions -Directory -Force | ForEach-Object { $_.Name })
    [Array]::Sort($names, [StringComparer]::Ordinal)
    [Array]::Reverse($names)
    $best = ''
    $any = ''
    foreach ($d in $names) {
        if ($d.StartsWith('native-', [StringComparison]::Ordinal) -or $d.StartsWith('.', [StringComparison]::Ordinal)) { continue }
        $dir = [IO.Path]::Combine($versions, $d)
        if (-not (Test-Path -LiteralPath ([IO.Path]::Combine($dir, '.installed')) -PathType Leaf)) { continue }
        $resources = [IO.Path]::Combine($dir, '.local', 'horizon-client')
        if (-not (Test-Path -LiteralPath $resources -PathType Container)) { continue }
        $prepared = @(Get-ChildItem -LiteralPath $resources -Directory -Force | Where-Object {
                Test-Path -LiteralPath ([IO.Path]::Combine($_.FullName, 'horizon-resources.json')) -PathType Leaf })
        if ($prepared.Count -eq 0) { continue }
        if (-not $any) { $any = $d }
        $lock = [IO.Path]::Combine($dir, 'research-lock.json')
        if (-not $best -and (Test-Path -LiteralPath $lock -PathType Leaf) -and
            ([IO.File]::ReadAllText($lock) -match ('"version": *"' + [regex]::Escape($Game) + '"'))) {
            $best = $d
        }
    }
    if ($best) { $best } else { $any }
}

# Copies a folder's contents but the game program and what marks a version installed.
function Copy-Tree([string]$From, [string]$To) {
    $skip = @('horizon-client', 'horizon-client.exe', '.installed', 'Horizon.app', 'native-mods.json')
    foreach ($entry in Get-ChildItem -LiteralPath $From -Force) {
        if ($skip -contains $entry.Name) { continue }
        Copy-Item -LiteralPath $entry.FullName -Destination ([IO.Path]::Combine($To, $entry.Name)) -Recurse -Force
    }
}

# ----- The prebuilt game -----
# $false: no prebuilt game to use, compile instead. A problem getting the prebuilt game falls back
# to compiling; one installing it stops (throws).

function Install-Prebuilt {
    if (-not $target -or -not $data) { return $false }
    if ($env:HORIZON_STUDIO_BUILD -eq '1') { return $false }
    $dir = [IO.Path]::Combine($cache, 'prebuilt')
    try {
        Remove-Folder $dir
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    } catch { return $false }
    $asset = "horizon-studio-game-$target.zip"
    $url = if ($env:HORIZON_STUDIO_RELEASE_URL) { $env:HORIZON_STUDIO_RELEASE_URL } else { "https://github.com/$REPO/releases/latest/download" }
    # Looked for quietly first: a release from before prebuilt games, or a build that failed, has none.
    try { Invoke-WebRequest "$url/$asset" -Method Head -UseBasicParsing | Out-Null }
    catch { Say "The latest release has no prebuilt game for $target."; return $false }
    Say "Downloading Horizon Studio, built for $target..."
    $zip = [IO.Path]::Combine($dir, $asset)
    try { Invoke-WebRequest "$url/$asset" -OutFile $zip -UseBasicParsing }
    catch { Say "Could not download $asset."; return $false }
    $sums = [IO.Path]::Combine($dir, 'SHA256SUMS')
    try { Invoke-WebRequest "$url/SHA256SUMS" -OutFile $sums -UseBasicParsing }
    catch { Say "The release has no checksums."; return $false }
    $want = [IO.File]::ReadAllLines($sums) |
        ForEach-Object { $p = $_.Trim() -split '\s+'; if ($p.Count -ge 2 -and $p[1] -eq $asset) { $p[0] } } |
        Select-Object -First 1
    $got = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $want -or $want -ne $got) { throw "$asset does not match the release's checksum." }
    try {
        Expand-Archive -LiteralPath $zip -DestinationPath $dir -Force
        Remove-Item -LiteralPath $zip -Force
    } catch { Say "Could not unpack $asset."; return $false }
    foreach ($need in @('horizon-client.exe', 'key-inputs', 'LICENSE-Horizon', "bundle/$ID/horizon-mixin.toml")) {
        if (-not (Test-Path -LiteralPath ([IO.Path]::Combine($dir, $need)) -PathType Leaf)) {
            Say "The prebuilt game has no $need."
            return $false
        }
    }
    # key-inputs: the horizon-mixins commit, profile, features, Rust, Horizon commit and game it was built with.
    $inputs = [IO.File]::ReadAllLines([IO.Path]::Combine($dir, 'key-inputs'))
    if ($inputs.Count -lt 6) { Say "The prebuilt game has no key-inputs."; return $false }
    $mixins, $buildProfile, $features, $rust, $revision, $game = $inputs[0..5]
    # The bundle as the launcher's registry ships a mod, with the commits the game was built with.
    $native = [IO.Path]::Combine($dir, 'native')
    $bundle = [IO.Path]::Combine($native, 'mixins', $ID)
    try {
        New-Item -ItemType Directory -Force -Path ([IO.Path]::Combine($native, 'mixins')) | Out-Null
        Move-Item -LiteralPath ([IO.Path]::Combine($dir, 'bundle', $ID)) -Destination $bundle
    } catch { return $false }
    $modversion = ''
    foreach ($line in [IO.File]::ReadAllLines([IO.Path]::Combine($bundle, 'horizon-mixin.toml'))) {
        if ($line -match '^version *= *"([^"]*)"') { $modversion = $Matches[1]; break }
    }
    try {
        Write-Text ([IO.Path]::Combine($native, 'horizon-native.json')) (
            '{"schema": 1, "id": "' + $ID + '", "version": "' + $modversion + '", "game": "' + $game +
            '", "horizon": "' + $revision + '", "mixins": "' + $mixins + '"}' + "`n")
    } catch { return $false }
    # The key names the launcher's Rust: this launcher's, which may be newer than the one it was built with.
    $theirs = ''
    try {
        $status = Invoke-Launcher @('--json', 'rust', 'status')
        if ($status.Out -match '"pinned": *"([^"]*)"') { $theirs = $Matches[1] }
    } catch { }
    $keyRust = if ($theirs) { $theirs } else { $rust }
    $version = Get-Key $bundle $mixins $buildProfile $features $keyRust $revision
    # Launchers that build with a quicker profile by default say which one goes in the name.
    $choose = @()
    try {
        if ((Invoke-Launcher @('native-build', '--help')).Out -match '--profile') { $choose = @('--profile', $buildProfile) }
    } catch { }
    $base = Get-BaseVersion $game
    if (-not $base) {
        throw "no prepared Horizon in the launcher yet: open it, install Horizon and play it once (or ``$launcher prepare``), then run this again."
    }
    $versions = [IO.Path]::Combine($data, 'versions')
    $seeded = [IO.Path]::Combine($versions, $version)
    if (-not (Test-Path -LiteralPath ([IO.Path]::Combine($seeded, '.installed')) -PathType Leaf)) {
        Say "Installing it as $version (on $base)..."
        $partial = [IO.Path]::Combine($versions, ".$version.partial")
        try {
            Remove-Folder $partial
            New-Item -ItemType Directory -Force -Path $partial | Out-Null
        } catch { throw "could not make $partial." }
        try {
            Copy-Tree ([IO.Path]::Combine($versions, $base)) $partial
            Copy-Item -LiteralPath ([IO.Path]::Combine($dir, 'horizon-client.exe')) -Destination ([IO.Path]::Combine($partial, 'horizon-client.exe')) -Force
            Copy-Item -LiteralPath ([IO.Path]::Combine($dir, 'LICENSE-Horizon')) -Destination ([IO.Path]::Combine($partial, 'LICENSE-Horizon')) -Force
            Write-Text ([IO.Path]::Combine($partial, 'native-mods.json')) (
                '{"base": "' + $base + '", "horizon": "' + $revision + '", "horizon_mixins": "' + $mixins +
                '", "rust": "' + $rust + '", "profile": "' + $buildProfile + '", "mods": [{"id": "' + $ID +
                '", "version": "' + $modversion + '"}], "prebuilt": "' + $target + '"}' + "`n")
        } catch {
            Remove-Folder $partial
            throw "could not install the game in $versions."
        }
        # Marked installed last, once whole: the launcher only lists folders with .installed.
        try {
            Remove-Folder $seeded
            Move-Item -LiteralPath $partial -Destination $seeded
            Write-Text ([IO.Path]::Combine($seeded, '.installed')) ''
        } catch { throw "could not install the game as $seeded." }
    }
    # The launcher's own step: it finds the build in place and selects it for an instance (the
    # selected one; older launchers make a Native mods instance, with your mod settings). A
    # launcher that names the build otherwise compiles instead.
    $run = Invoke-Launcher (@('native-build', $native, '--version', $base) + $choose + @('--trust-native-code'))
    if ($run.Code -ne 0) {
        if ($run.Out) { Say $run.Out }
        throw "the launcher's native-build failed (exit code $($run.Code))."
    }
    Say $run.Out
    if ($run.Out.StartsWith("$version ", [StringComparison]::Ordinal)) {
        Say 'Done: open the launcher and press Play (older launchers: on the Native mods instance).'
    } else {
        Say 'Your launcher compiled it instead (it names this build differently); open the launcher and press Play.'
    }
    return $true
}

# ----- Compiling, as before -----

function Install-ByCompiling {
    Say 'Compiling Horizon Studio into the game (the first time takes a while; later ones are faster)...'
    $src = [IO.Path]::Combine($cache, 'horizon-studio-main')
    $zip = [IO.Path]::Combine($cache, 'horizon-studio-main.zip')
    try {
        New-Item -ItemType Directory -Force -Path $cache | Out-Null
        Remove-Folder $src
        Invoke-WebRequest "https://github.com/$REPO/archive/refs/heads/main.zip" -OutFile $zip -UseBasicParsing
        Expand-Archive -LiteralPath $zip -DestinationPath $cache -Force
        Remove-Item -LiteralPath $zip -Force
    } catch { throw "could not download Horizon Studio's source." }
    # Optimised as the release profile is, without its slow whole-program steps: the game
    # compiles and links several times faster, and a rebuild only redoes what changed.
    $faster = @{}
    foreach ($pair in @(@('CARGO_PROFILE_RELEASE_LTO', 'false'), @('CARGO_PROFILE_RELEASE_CODEGEN_UNITS', '16'),
                        @('CARGO_PROFILE_RELEASE_INCREMENTAL', 'true'))) {
        $set = [Environment]::GetEnvironmentVariable($pair[0])
        $faster[$pair[0]] = if ($set) { $set } else { $pair[1] }
    }
    $run = Invoke-Launcher @('native-build', [IO.Path]::Combine($src, 'mixins', $ID), '--trust-native-code') -Environment $faster
    if ($run.Out) { Say $run.Out }
    if ($run.Code -ne 0) { throw "the launcher's native-build failed (exit code $($run.Code))." }
    Say 'Done: open the launcher and press Play (older launchers: on the Native mods instance).'
}

try {
    if (-not $launcher) {
        throw 'Horizon''s launcher (horizon-launcher.exe) was not found. Get it from https://github.com/horizon-rust/horizon/releases?q=launcher-v (horizon-launcher-windows-x64.zip), unpack it and open it once, then run this again; or set HORIZON_LAUNCHER to the path of horizon-launcher.exe.'
    }
    Wait-LauncherClosed
    # The last value is Install-Prebuilt's answer.
    if (@(Install-Prebuilt)[-1] -ne $true) { Install-ByCompiling }
} catch {
    Write-Host "error: $($_.Exception.Message)" -ForegroundColor Red
    # Run as a file (powershell -File install.ps1): fail with an exit code. Through `irm | iex`,
    # exiting would close the window, so it only stops.
    if ($PSCommandPath) { exit 1 }
}
} @args
