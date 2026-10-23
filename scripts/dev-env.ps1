# Optional project-local toolchain. Standard Visual Studio/Rust installations also work.
$repoRoot = Split-Path -Parent $PSScriptRoot
$localCargo = Join-Path $repoRoot '.tools/cargo'
if (Test-Path -LiteralPath $localCargo) {
    $env:CARGO_HOME = $localCargo
    $env:RUSTUP_HOME = Join-Path $repoRoot '.tools/rustup'
    $env:PATH = (Join-Path $localCargo 'bin') + ';' + $env:PATH
}
$localMsvc = Join-Path $repoRoot '.tools/msvc/VC/Tools/MSVC/14.44.35207'
$localSdk = Join-Path $repoRoot '.tools/sdk/Windows Kits/10'
if ((Test-Path -LiteralPath $localMsvc) -and (Test-Path -LiteralPath $localSdk)) {
    $sdkVersion = '10.0.26100.0'
    $env:PATH = (Join-Path $localMsvc 'bin/Hostx64/x64') + ';' + (Join-Path $localSdk "bin/$sdkVersion/x64") + ';' + $env:PATH
    $env:INCLUDE = (Join-Path $localMsvc 'include') + ';' + (Join-Path $localSdk "Include/$sdkVersion/ucrt") + ';' + (Join-Path $localSdk "Include/$sdkVersion/um") + ';' + (Join-Path $localSdk "Include/$sdkVersion/shared")
    $env:LIB = (Join-Path $localMsvc 'lib/x64') + ';' + (Join-Path $localMsvc 'lib/onecore/x64') + ';' + (Join-Path $localSdk "Lib/$sdkVersion/ucrt/x64") + ';' + (Join-Path $localSdk "Lib/$sdkVersion/um/x64")
    $env:CC = Join-Path $localMsvc 'bin/Hostx64/x64/cl.exe'
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $localMsvc 'bin/Hostx64/x64/link.exe'
}
