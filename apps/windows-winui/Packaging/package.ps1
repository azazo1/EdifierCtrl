param([ValidateSet('build', 'run', 'debug', 'dist')][string]$Mode = 'build')
$ErrorActionPreference = 'Stop'

function Resolve-DotnetSdk {
    $candidates = @((Get-Command dotnet -All -ErrorAction SilentlyContinue | Where-Object CommandType -eq 'Application').Source)
    if (Get-Command scoop -ErrorAction SilentlyContinue) {
        $prefix = scoop prefix dotnet-sdk 2>$null
        if ($LASTEXITCODE -eq 0 -and $prefix) { $candidates += Join-Path ([string]$prefix) 'dotnet.exe' }
    }
    foreach ($candidate in ($candidates | Select-Object -Unique)) {
        if (-not $candidate -or -not (Test-Path $candidate)) { continue }
        $sdks = & $candidate --list-sdks
        if ($LASTEXITCODE -eq 0 -and ($sdks -match '^\d+\.\d+\.\d+')) { return $candidate }
    }
    throw '未找到 .NET SDK. 请指定包含 SDK 的 dotnet 路径后重试.'
}

$dotnet = Resolve-DotnetSdk
$root = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
Push-Location $root
try {
    $configuration = if ($Mode -eq 'dist') { 'Release' } else { 'Debug' }
    $profile = if ($Mode -eq 'dist') { 'release' } else { 'debug' }
    Write-Host "[1/3] 构建 Rust 核心 ($profile)"
    if ($Mode -eq 'dist') { cargo build -p edifier-ffi --release --locked } else { cargo build -p edifier-ffi --locked }
    if ($LASTEXITCODE -ne 0) { throw 'Rust 核心构建失败.' }
    & (Join-Path $PSScriptRoot 'generate-icon.ps1') -OutputPath (Join-Path $root 'target/windows/assets/AppIcon.ico')
    Write-Host "[2/3] 构建 WinUI ($configuration)"
    $project = Join-Path $root 'apps/windows-winui/EdifierCtrl.csproj'
    $version = if ($env:APP_VERSION) { $env:APP_VERSION } else { 'dev-build' }
    if ($Mode -eq 'dist') {
        $output = Join-Path $root 'target/windows/dist/EdifierCtrl'
        & $dotnet publish $project -c Release -p:Platform=x64 -r win-x64 --self-contained true -p:EdifierVersion=$version -o $output
        if ($LASTEXITCODE -ne 0) { throw 'WinUI 发布构建失败.' }
        foreach ($required in @('EdifierCtrl.exe', 'edifier_ffi.dll', 'resources.pri')) {
            if (-not (Test-Path (Join-Path $output $required))) { throw "分发文件缺失: $required" }
        }
        Write-Host '[3/3] 打包 Windows 便携应用'
        $archive = Join-Path $root "target/windows/dist/EdifierCtrl-$version-windows-x86_64.zip"
        Compress-Archive -Path "$output/*" -DestinationPath $archive -Force
        $hash = (Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText((Join-Path $root 'target/windows/dist/SHA256SUMS'), "$hash  $([IO.Path]::GetFileName($archive))`n", [Text.UTF8Encoding]::new($false))
        Write-Host "应用: $output/EdifierCtrl.exe"
        Write-Host "便携包: $archive"
    } else {
        & $dotnet build $project -c Debug -p:Platform=x64 -p:EdifierVersion=$version
        if ($LASTEXITCODE -ne 0) { throw 'WinUI 构建失败.' }
        $executable = Join-Path $root 'apps/windows-winui/bin/x64/Debug/net8.0-windows10.0.19041.0/EdifierCtrl.exe'
        Write-Host "[3/3] 应用已就绪: $executable"
        if ($Mode -eq 'debug') {
            $env:EDIFIER_DATA_DIR = Join-Path $root 'target/edifierctrl-windows-debug'
            $env:EDIFIER_LOG_FILE = Join-Path $env:EDIFIER_DATA_DIR 'app.log'
            $env:EDIFIER_LOG_LEVEL = 'trace'
        }
        if ($Mode -in @('run', 'debug')) { & $executable }
    }
} finally { Pop-Location }
