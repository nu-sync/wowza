param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '../target/dist')
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Build the Windows package on Windows.' }
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$destination = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $destination) { throw 'Choose a new output directory; existing packages are never overwritten.' }
$target = 'x86_64-pc-windows-msvc'
$originalFlags = $env:CARGO_ENCODED_RUSTFLAGS
Push-Location $repository
try {
    $metadata = cargo metadata --locked --no-default-features --format-version 1 --filter-platform $target | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read locked package metadata.' }
    $package = $metadata.packages | Where-Object { $_.name -eq 'wowza' -and -not $_.source }
    $rustRoot = rustc --print sysroot
    if ($LASTEXITCODE -ne 0) { throw 'Cannot locate the Rust toolchain.' }
    $rustNotices = Join-Path $rustRoot 'share/doc/rust'
    if (-not (Test-Path -LiteralPath (Join-Path $rustNotices 'COPYRIGHT-library.html'))) {
        throw 'Rust standard-library notices are missing from the installed toolchain.'
    }
    $flags = @("--remap-path-prefix=$repository=wowza")
    if ($env:USERPROFILE) { $flags += "--remap-path-prefix=$env:USERPROFILE=home" }
    if ($env:CARGO_HOME) { $flags += "--remap-path-prefix=$env:CARGO_HOME=cargo" }
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]31
    cargo build --locked --release --no-default-features --target $target
    if ($LASTEXITCODE -ne 0) { throw 'Windows release build failed.' }

    $name = "wowza-$($package.version)-windows-x86_64"
    $stage = Join-Path $destination $name
    New-Item -ItemType Directory -Path $stage | Out-Null
    $binary = Join-Path $metadata.target_directory "$target/release/wowza.exe"
    Copy-Item -LiteralPath $binary -Destination (Join-Path $stage 'wowza.exe')
    foreach ($file in @('README.md', 'LICENSE', 'AGENTS.md')) {
        Copy-Item -LiteralPath (Join-Path $repository $file) -Destination (Join-Path $stage $file)
    }
    New-Item -ItemType Directory -Path (Join-Path $stage 'docs') | Out-Null
    Copy-Item -LiteralPath (Join-Path $repository 'docs/REFERENCE.md') -Destination (Join-Path $stage 'docs/REFERENCE.md')

    $notices = [Text.StringBuilder]::new()
    [void]$notices.AppendLine('# Third-party dependency notices')
    [void]$notices.AppendLine('License and notice files from the locked Windows dependency sources follow. Rust standard-library notices are included separately in rust-notices/.')
    foreach ($dependency in ($metadata.packages | Where-Object { $_.source } | Sort-Object name, version)) {
        $directory = Split-Path -Parent $dependency.manifest_path
        $files = @(Get-ChildItem -LiteralPath $directory -File |
            Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)' } |
            Select-Object -ExpandProperty FullName)
        if ($dependency.license_file) { $files += Join-Path $directory $dependency.license_file }
        $files = @($files | Sort-Object -Unique)
        if ($files.Count -eq 0) { throw "No license notices found for $($dependency.name)." }
        [void]$notices.AppendLine("`n## $($dependency.name) $($dependency.version)")
        [void]$notices.AppendLine("Declared license: $($dependency.license)")
        foreach ($file in $files) {
            [void]$notices.AppendLine("`n### $([IO.Path]::GetFileName($file))`n")
            [void]$notices.AppendLine([IO.File]::ReadAllText($file))
        }
    }
    [IO.File]::WriteAllText((Join-Path $stage 'THIRD_PARTY_LICENSES.md'), $notices.ToString(), [Text.UTF8Encoding]::new($false))
    $runtimeNotices = Join-Path $stage 'rust-notices'
    New-Item -ItemType Directory -Path $runtimeNotices | Out-Null
    Copy-Item -LiteralPath (Join-Path $rustNotices 'COPYRIGHT-library.html') -Destination $runtimeNotices
    Copy-Item -LiteralPath (Join-Path $rustNotices 'licenses') -Destination $runtimeNotices -Recurse

    $archive = Join-Path $destination "$name.zip"
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive
    $checksum = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $destination 'SHA256SUMS'), "$checksum  $name.zip`n", [Text.UTF8Encoding]::new($false))
    Write-Output "Windows package: $archive"
    Write-Output "Checksums: $(Join-Path $destination 'SHA256SUMS')"
} finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $originalFlags
    Pop-Location
}
