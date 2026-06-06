# Build RedLight as a release .exe using the MinGW (GNU) toolchain.
# MinGW's dlltool/as/ld must be on PATH; this prepends your install.
$mingwBin = "C:\Users\Roshan\tools\mingw64\bin"
if (Test-Path $mingwBin) {
    $env:PATH = "$mingwBin;$env:PATH"
} else {
    Write-Warning "MinGW not found at $mingwBin - edit this path in build.ps1"
}

cargo build --release
if ($LASTEXITCODE -eq 0) {
    $exe = Join-Path $PSScriptRoot "target\release\RedLight.exe"
    Write-Host ""
    Write-Host "Built: $exe" -ForegroundColor Green
}
