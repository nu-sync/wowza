$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed.' }
    cargo clippy --locked --all-targets --all-features -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Lint check failed.' }
    cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw 'Default tests failed.' }
    cargo test --locked --features test-hooks
    if ($LASTEXITCODE -ne 0) { throw 'Interruption tests failed.' }
    cargo build --locked --release
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
} finally {
    Pop-Location
}
