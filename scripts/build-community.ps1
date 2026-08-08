#requires -Version 5.1
<#
.SYNOPSIS
  一键编译 SoundLink 免费版（社区版）三种 Windows 产物。

.DESCRIPTION
  在公开仓库默认状态（desktop/pro 为免费实现，EDITION=community）下执行：
    1. 校验当前为免费实现（无 pro-free-backup 残留、EDITION=community）
    2. 前端依赖就绪（desktop/ui/node_modules 缺失时自动 npm ci）
    3. tauri build --features tauri_app --bundles nsis,msi
    4. 收集产物到 dist/community/

  三种产物（便携 exe 直接复用 target/release/soundlink.exe，无需二次编译）：
    - NSIS 安装包  : dist/community/*-setup.exe
    - MSI 安装包   : dist/community/*.msi
    - 绿色便携 exe : dist/community/SoundLink_<版本>_x64-portable.exe

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\build-community.ps1
#>
[CmdletBinding()]
param(
    # node_modules 已就绪时传此开关可跳过前端依赖检查
    [switch]$SkipUiInstall
)

$ErrorActionPreference = 'Stop'

$OssRoot   = Split-Path -Parent $PSScriptRoot
$Desktop   = Join-Path $OssRoot 'desktop'
$ProDir    = Join-Path $Desktop 'pro'
$BackupDir = Join-Path $Desktop 'pro-free-backup'
$SrcTauri  = Join-Path $Desktop 'src-tauri'
$UiDir     = Join-Path $Desktop 'ui'
$Version   = (Get-Content (Join-Path $OssRoot 'VERSION') -Raw).Trim()
$DistDir   = Join-Path $OssRoot 'dist\community'

function Get-ProEdition {
    $lib = Join-Path $ProDir 'src\lib.rs'
    $m = Select-String -Path $lib -Pattern 'EDITION: &str = "(\w+)"' | Select-Object -First 1
    if (-not $m) { throw "无法从 $lib 读取 EDITION。" }
    return $m.Matches[0].Groups[1].Value
}

function Invoke-Native {
    param([string]$Exe, [string[]]$CmdArgs, [string]$WorkDir)
    Push-Location $WorkDir
    try {
        & $Exe @CmdArgs
        if ($LASTEXITCODE -ne 0) { throw "命令失败（exit $LASTEXITCODE）：$Exe $($CmdArgs -join ' ')" }
    } finally {
        Pop-Location
    }
}

function Clear-SoundlinkProCache {
    <#
    .SYNOPSIS
      物理清除 soundlink-pro 的 Cargo 构建缓存（与 build-pro.ps1 同款逻辑）。
      ⚠ 不能用 `cargo clean -p soundlink-pro`：当 desktop/pro 内容被整体替换后，
      Cargo 按当前源码指纹匹配不到上一次编译产物，会报 `Removed 0 files` 并留下旧 rlib，
      导致下一次链接静默复用旧实现（09 文档 §11 V-8 同源问题，物理替换场景同样会发生）。
      这里直接删 target/release 下所有 soundlink-pro 相关残留，让 Cargo 必然重编。
    #>
    $releaseDir = Join-Path $SrcTauri 'target\release'
    if (-not (Test-Path $releaseDir)) { return }
    $removed = 0
    foreach ($pat in 'libsoundlink_pro*', 'soundlink_pro*') {
        Get-ChildItem (Join-Path $releaseDir 'deps') -Filter $pat -ErrorAction SilentlyContinue |
            ForEach-Object { Remove-Item $_.FullName -Recurse -Force; $removed++ }
    }
    foreach ($pat in 'soundlink-pro-*', 'soundlink-pro-api-*') {
        Get-ChildItem (Join-Path $releaseDir '.fingerprint') -Directory -Filter $pat -ErrorAction SilentlyContinue |
            ForEach-Object { Remove-Item $_.FullName -Recurse -Force; $removed++ }
    }
    Get-ChildItem (Join-Path $releaseDir 'incremental') -Directory -Filter 'soundlink_pro*' -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item $_.FullName -Recurse -Force; $removed++ }
    if ($removed -gt 0) { Write-Host "    已物理清除 soundlink-pro 缓存残留 $removed 处" }
}

# --- 1. 校验仓库状态 -------------------------------------------------------
Write-Host "==> [1/4] 校验仓库状态（应为免费实现）" -ForegroundColor Cyan
if (Test-Path $BackupDir) {
    throw "检测到 desktop/pro-free-backup：仓库处于 Pro 切换状态（或上次 Pro 构建未还原）。请先还原为免费版（见 docs/user/09-open-core-build.md §4.3.1）再构建。"
}
$edition = Get-ProEdition
if ($edition -ne 'community') {
    throw "desktop/pro 当前 EDITION=$edition（期望 community）。请先还原为免费实现再构建免费版。"
}

# --- 2. 前端依赖 -----------------------------------------------------------
Write-Host "==> [2/5] 前端依赖" -ForegroundColor Cyan
if (-not $SkipUiInstall -and -not (Test-Path (Join-Path $UiDir 'node_modules'))) {
    Invoke-Native npm @('ci') $UiDir
} else {
    Write-Host "    node_modules 已就绪，跳过 npm ci"
}

# --- 3. 清缓存（防上次 Pro 构建残留污染） -----------------------------------
Write-Host "==> [3/5] 物理清除 soundlink-pro 缓存（防上一次 Pro 构建残留）" -ForegroundColor Cyan
Clear-SoundlinkProCache

# --- 4. 构建（NSIS + MSI，同步产出 release exe） ----------------------------
Write-Host "==> [4/5] tauri build --features tauri_app --bundles nsis,msi" -ForegroundColor Cyan
Invoke-Native npm @('exec', '--prefix', '..\ui', 'tauri', '--', 'build', '--features', 'tauri_app', '--bundles', 'nsis,msi') $SrcTauri

# --- 5. 收集产物 -----------------------------------------------------------
Write-Host "==> [5/5] 收集产物到 $DistDir" -ForegroundColor Cyan
$bundleDir = Join-Path $SrcTauri 'target\release\bundle'
$nsis = Get-ChildItem -Path (Join-Path $bundleDir 'nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue
$msi  = Get-ChildItem -Path (Join-Path $bundleDir 'msi')  -Filter '*.msi'        -ErrorAction SilentlyContinue
$exe  = Join-Path $SrcTauri 'target\release\soundlink.exe'
if (-not $nsis) { throw "未找到 NSIS 产物（$bundleDir\nsis）。" }
if (-not $msi)  { throw "未找到 MSI 产物（$bundleDir\msi）。" }
if (-not (Test-Path $exe)) { throw "未找到便携 exe：$exe" }

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
$nsis | Copy-Item -Destination $DistDir -Force
$msi  | Copy-Item -Destination $DistDir -Force
$portable = Join-Path $DistDir "SoundLink_${Version}_x64-portable.exe"
Copy-Item $exe $portable -Force

Write-Host ""
Write-Host "免费版构建完成（v$Version，EDITION=community）：" -ForegroundColor Green
Get-ChildItem $DistDir | ForEach-Object { Write-Host "  $($_.FullName)" }
