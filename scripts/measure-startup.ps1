#requires -Version 7.0
<#
.SYNOPSIS
Measures process launch through exit, with captured output and alternating order.
.EXAMPLE
./scripts/measure-startup.ps1 -Executable ./target/latency/wowza-before.exe,./target/release/wowza.exe,wowmc -Name before,after,wowmc
.EXAMPLE
./scripts/measure-startup.ps1 -Executable ./target/latency/wowza-before.exe,./target/release/wowza.exe,wowmc -Name before,after,wowmc -ArgumentList --help -OutputPath ./target/latency/help.json
#>
[CmdletBinding()]
param(
    [string[]]$Executable = @((Join-Path $PSScriptRoot '../target/release/wowza.exe')),
    [string[]]$Name = @(),
    [string[]]$ArgumentList = @(),
    [hashtable]$Environment = @{},
    [ValidateRange(5, 10000)][int]$Rounds = 20,
    [ValidateRange(1, 100)][int]$Warmups = 3,
    [ValidateRange(1, 300)][int]$TimeoutSeconds = 30,
    [string]$OutputPath = (Join-Path $PSScriptRoot '../target/latency/startup.json')
)

$ErrorActionPreference = 'Stop'
if ($Executable.Count -eq 0) { throw 'Provide at least one executable.' }
if ($Name.Count -ne 0 -and $Name.Count -ne $Executable.Count) {
    throw 'Provide one name per executable, or omit names.'
}

$cases = @(for ($i = 0; $i -lt $Executable.Count; $i++) {
    $path = if (Test-Path -LiteralPath $Executable[$i] -PathType Leaf) {
        (Resolve-Path -LiteralPath $Executable[$i]).Path
    } else {
        (Get-Command $Executable[$i] -CommandType Application -ErrorAction Stop).Source
    }
    [pscustomobject]@{
        Name = $(if ($Name.Count) { $Name[$i] } else { [IO.Path]::GetFileNameWithoutExtension($path) })
        Path = $path
        Sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        Output = $null
    }
})
if (@($cases.Name | Select-Object -Unique).Count -ne $cases.Count) {
    throw 'Executable names must be unique; supply -Name when basenames repeat.'
}

function Invoke-Sample($case) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $case.Path
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.RedirectStandardInput = $true
    foreach ($argument in $ArgumentList) { $info.ArgumentList.Add($argument) }
    foreach ($key in $Environment.Keys) { $info.Environment[$key] = $Environment[$key] }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    try {
        $watch = [Diagnostics.Stopwatch]::StartNew()
        if (-not $process.Start()) { throw "Cannot start $($case.Name)." }
        $process.StandardInput.Close()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw "$($case.Name) exceeded $TimeoutSeconds seconds. No timing result accepted."
        }
        $out = $stdout.GetAwaiter().GetResult()
        $err = $stderr.GetAwaiter().GetResult()
        $watch.Stop()
        if ($process.ExitCode -ne 0 -or $err.Length -ne 0 -or $out.Length -eq 0) {
            throw "$($case.Name) failed validation (exit $($process.ExitCode)): $err"
        }
        if ($null -eq $case.Output) { $case.Output = $out }
        elseif ($case.Output -cne $out) { throw "$($case.Name) output changed between samples." }
        return $watch.Elapsed.TotalMilliseconds
    } finally {
        $process.Dispose()
    }
}

for ($warmup = 0; $warmup -lt $Warmups; $warmup++) {
    foreach ($case in $cases) { $null = Invoke-Sample $case }
}
$samples = @(for ($round = 0; $round -lt $Rounds; $round++) {
    $order = @($cases)
    if ($round % 2 -ne 0) { [array]::Reverse($order) }
    foreach ($case in $order) {
        [pscustomobject]@{ Round = $round + 1; Name = $case.Name; Milliseconds = (Invoke-Sample $case) }
    }
})
$summary = @(foreach ($case in $cases) {
    $values = @($samples | Where-Object Name -EQ $case.Name | Sort-Object Milliseconds | ForEach-Object Milliseconds)
    $middle = [int][math]::Floor($values.Count / 2)
    $median = if ($values.Count % 2) { $values[$middle] } else { ($values[$middle - 1] + $values[$middle]) / 2 }
    [pscustomobject]@{
        Name = $case.Name
        Samples = $values.Count
        MedianMs = [math]::Round($median, 3)
        MinMs = [math]::Round($values[0], 3)
        MaxMs = [math]::Round($values[-1], 3)
    }
})
$result = [ordered]@{
    CapturedAtUtc = [datetime]::UtcNow.ToString('o')
    Metric = 'Wall-clock process start through exit and output drain; console rendering excluded'
    OperatingSystem = [Runtime.InteropServices.RuntimeInformation]::OSDescription
    Architecture = [Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
    PowerShellVersion = $PSVersionTable.PSVersion.ToString()
    Warmups = $Warmups
    Arguments = $ArgumentList
    EnvironmentOverrides = $Environment
    Failures = 0
    Cases = $cases
    Summary = $summary
    Samples = $samples
}
$outputFile = [IO.Path]::GetFullPath($OutputPath)
$null = New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($outputFile))
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputFile -Encoding utf8
$summary | Format-Table -AutoSize
Write-Host "Raw samples and validated output: $outputFile"
