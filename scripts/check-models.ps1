param(
    [Parameter(Mandatory = $true)][string]$TlaTools,
    [string]$StatechartScripts,
    [string]$OutputDirectory = (Join-Path ([IO.Path]::GetTempPath()) ('wowza-models-' + [guid]::NewGuid().ToString('N')))
)
$ErrorActionPreference = 'Stop'
$repository = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\', '/')
$destination = [IO.Path]::GetFullPath($OutputDirectory)
if ($destination -eq $repository -or $destination.StartsWith($repository + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Use a new scratch directory outside the repository for model checking.'
}
if (Test-Path -LiteralPath $destination) { throw 'The output directory must be new.' }
$jar = (Resolve-Path -LiteralPath $TlaTools).Path
$worker = if ($StatechartScripts) {
    (Resolve-Path -LiteralPath (Join-Path $StatechartScripts 'worker.ts')).Path
}
$originalJar = $env:TLA2TOOLS
New-Item -ItemType Directory -Path $destination | Out-Null
Get-ChildItem -LiteralPath (Join-Path $repository 'models') -File |
    Where-Object { $_.Extension -in @('.tla', '.cfg', '.mjs') } |
    Copy-Item -Destination $destination
Push-Location $destination
try {
    $env:TLA2TOOLS = $jar
    foreach ($model in @('Lifecycle', 'Update', 'Handoff', 'Plan', 'RecoveryChoice', 'InstallationScope', 'RecoveryPreflight')) {
        java -XX:+UseParallelGC -cp $jar tlc2.TLC -workers 1 -fp 0 -metadir "$model-states" -config "$model.cfg" "$model.tla" 2>&1 |
            Set-Content -LiteralPath "$model.out"
        if ($LASTEXITCODE -ne 0) { throw "$model model failed; see $destination/$model.out" }
        Write-Output "$model passed."
    }
    if ($worker) {
        foreach ($model in @('Plan', 'RecoveryChoice', 'InstallationScope', 'RecoveryPreflight')) {
            New-Item -ItemType Directory -Path $model | Out-Null
            node $worker $model "$model.tla" "$model.cfg" "$model.view.mjs"
            if ($LASTEXITCODE -ne 0) { throw "$model statechart failed." }
            Get-Content -LiteralPath "$model/done/report.md"
        }
    }
    foreach ($case in @(
        @{ Model = 'InstallationScope'; Rule = 'SameInstallation' },
        @{ Model = 'RecoveryPreflight'; Rule = 'HonestPreview' }
    )) {
        $model = $case.Model
        (Get-Content -LiteralPath "$model.cfg" -Raw).Replace('= TRUE', '= FALSE') |
            Set-Content -LiteralPath "$model-before.cfg"
        java -XX:+UseParallelGC -cp $jar tlc2.TLC -workers 1 -fp 0 -metadir "$model-before-states" -config "$model-before.cfg" "$model.tla" 2>&1 |
            Set-Content -LiteralPath "$model-before.out"
        if ($LASTEXITCODE -ne 12 -or -not (Select-String -LiteralPath "$model-before.out" -SimpleMatch "Invariant $($case.Rule) is violated.")) {
            throw "$model did not reproduce its pre-fix invariant violation."
        }
    }
    $source = Get-Content -LiteralPath 'Plan.tla' -Raw
    $guard = 'phase = "confirmed" /\ revision = expectedRevision'
    if (-not $source.Contains($guard)) { throw 'The negative-control mutation no longer matches the model.' }
    $negative = $source.Replace('MODULE Plan', 'MODULE PlanNegative').Replace($guard, 'phase = "confirmed"')
    Set-Content -LiteralPath 'PlanNegative.tla' -Value $negative
    java -XX:+UseParallelGC -cp $jar tlc2.TLC -workers 1 -fp 0 -metadir 'negative-states' -config Plan.cfg PlanNegative.tla 2>&1 |
        Set-Content -LiteralPath 'negative-control.out'
    if ($LASTEXITCODE -ne 12 -or -not (Select-String -LiteralPath 'negative-control.out' -SimpleMatch 'Invariant FreshInputsRequired is violated.')) {
        throw 'Negative control did not detect stale-input execution.'
    }
    $global:LASTEXITCODE = 0
    Write-Output "Seven models passed; both pre-fix bugs and the stale-input negative control reproduced. Reports: $destination"
} finally {
    $env:TLA2TOOLS = $originalJar
    Pop-Location
}
