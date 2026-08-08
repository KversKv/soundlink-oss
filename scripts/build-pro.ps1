#requires -Version 5.1
<#
.SYNOPSIS
  一键编译 SoundLink Pro 版（官方版）三种 Windows 产物，完成后自动把仓库还原为免费版。

.DESCRIPTION
  按 docs/user/09-open-core-build.md §4.2 方式 A（物理替换）执行：
    1. 校验前置条件（私有仓库存在、当前为免费实现、无 pro-free-backup 残留）
    2. desktop/pro 重命名为 pro-free-backup，复制私有实现（仅 Cargo.toml + src/）到 desktop/pro
    3. cargo clean -p soundlink-pro（红线 G10：替换后必须清缓存）
    4. tauri build --features tauri_app --bundles nsis,msi，收集产物到 dist/pro/
    5. 【try/finally 保证】删除私有副本、恢复免费实现、再次 cargo clean -p soundlink-pro，
       并校验 EDITION 已回到 community——即使构建失败也会还原

  三种产物：
    - NSIS 安装包  : dist/pro/*-setup.exe
    - MSI 安装包   : dist/pro/*.msi
    - 绿色便携 exe : dist/pro/SoundLink_<版本>_x64-portable.exe

  说明：私有仓库里的 license/（含 vendor 私钥）不会复制进公开仓库，只复制构建所需的 crate 文件。

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\build-pro.ps1
#>
[CmdletBinding()]
param(
    # node_modules 已就绪时传此开关可跳过前端依赖检查
    [switch]$SkipUiInstall
)

$ErrorActionPreference = 'Stop'

$OssRoot   = Split-Path -Parent $PSScriptRoot
$ProRepo   = Join-Path (Split-Path -Parent $OssRoot) 'pro'   # 与 oss 平级的私有仓库
$Desktop   = Join-Path $OssRoot 'desktop'
$ProDir    = Join-Path $Desktop 'pro'
$BackupDir = Join-Path $Desktop 'pro-free-backup'
$SrcTauri  = Join-Path $Desktop 'src-tauri'
$UiDir     = Join-Path $Desktop 'ui'
$Version   = (Get-Content (Join-Path $OssRoot 'VERSION') -Raw).Trim()
$DistDir   = Join-Path $OssRoot 'dist\pro'

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
      物理清除 soundlink-pro 的 Cargo 构建缓存。
      ⚠ 不能用 `cargo clean -p soundlink-pro`：当 desktop/pro 的目录内容被整体替换后，
      Cargo 判定时基于的是「当前磁盘源码的指纹」，与上一次编译产物指纹不匹配，
      会报 `Removed 0 files` 并把旧 rlib 留在 target 里，导致下一次链接静默复用旧实现
      （09 文档 §11 V-8 同源问题，物理替换场景下同样会发生）。
      这里直接删 target/release 下所有 soundlink-pro 相关残留，让 Cargo 必然重编。
    #>
    $releaseDir = Join-Path $SrcTauri 'target\release'
    if (-not (Test-Path $releaseDir)) { return }
    $removed = 0
    # deps 下的 rlib / rmeta / d 文件（含 soundlink-pro 与 soundlink-pro-api，
    # 两者同源耦合，一并清掉最稳妥）
    foreach ($pat in 'libsoundlink_pro*', 'soundlink_pro*') {
        Get-ChildItem (Join-Path $releaseDir 'deps') -Filter $pat -ErrorAction SilentlyContinue |
            ForEach-Object { Remove-Item $_.FullName -Recurse -Force; $removed++ }
    }
    # .fingerprint 下的指纹目录（cargo 增量判定依据）
    foreach ($pat in 'soundlink-pro-*', 'soundlink-pro-api-*') {
        Get-ChildItem (Join-Path $releaseDir '.fingerprint') -Directory -Filter $pat -ErrorAction SilentlyContinue |
            ForEach-Object { Remove-Item $_.FullName -Recurse -Force; $removed++ }
    }
    # incremental 缓存（可能保留旧的 MIR/object）
    Get-ChildItem (Join-Path $releaseDir 'incremental') -Directory -Filter 'soundlink_pro*' -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item $_.FullName -Recurse -Force; $removed++ }
    Write-Host "    已物理清除 soundlink-pro 缓存残留 $removed 处"
}

function Test-TargetPathConsistent {
    <#
    .SYNOPSIS
      检测 target 目录是否从其他路径搬迁过来（路径不一致）。

    .DESCRIPTION
      Cargo 的 fingerprint 基于源码内容 hash，不检测 target 绝对路径变化。
      若仓库被重命名/移动（如 SoundLink→Soundlink、或嵌入 oss 子目录），
      target 内 build script 的 root-output / output 仍保存旧路径，
      Cargo 会复用旧的环境变量（如 tauri 的 CORE_PLUGIN___PERMISSION_FILES_PATH），
      导致 tauri_build::build() 读取不存在的旧路径而失败：
        failed to read plugin permissions: ... 系统找不到指定的路径。 (os error 3)
      检测到不一致时返回 $false，调用方应执行 cargo clean。
    #>
    $buildDir = Join-Path $SrcTauri 'target\release\build'
    if (-not (Test-Path $buildDir)) { return $true }

    # 抽样 tauri build script 的 root-output（单行，即 OUT_DIR）
    $tauriDirs = Get-ChildItem $buildDir -Directory -Filter 'tauri-*' -ErrorAction SilentlyContinue
    foreach ($dir in $tauriDirs) {
        $rootOutput = Join-Path $dir.FullName 'root-output'
        if (Test-Path $rootOutput) {
            $content = (Get-Content $rootOutput -Raw -ErrorAction SilentlyContinue).Trim()
            if ($content -and -not $content.StartsWith($SrcTauri, [System.StringComparison]::OrdinalIgnoreCase)) {
                return $false
            }
        }
    }
    return $true
}

# --- 1. 前置校验 -----------------------------------------------------------
Write-Host "==> [1/5] 前置校验" -ForegroundColor Cyan
if (-not (Test-Path (Join-Path $ProRepo 'Cargo.toml')) -or -not (Test-Path (Join-Path $ProRepo 'src\lib.rs'))) {
    throw "未找到私有仓库 $ProRepo（期望与 oss 平级，含 Cargo.toml 与 src\lib.rs）。"
}
if (Test-Path $BackupDir) {
    throw "检测到 desktop/pro-free-backup：仓库已处于 Pro 切换状态（或上次构建未还原）。请手动处理：删除 desktop\pro 后将 pro-free-backup 重命名回 pro。"
}
if ((Get-ProEdition) -ne 'community') {
    throw "desktop/pro 当前 EDITION 不是 community，无法安全切换。请先恢复免费实现。"
}

# 检测 target 目录是否从其他路径搬迁过来。Cargo fingerprint 不检测绝对路径变化，
# 若仓库被重命名/移动，target 内 build script 缓存的 root-output 仍指向旧路径，
# tauri_build::build() 读取旧路径下的 permissions/*.toml 会失败（os error 3）。
# 必须先 cargo clean 才能继续。
if (-not (Test-TargetPathConsistent)) {
    Write-Host "    检测到 target 缓存路径与当前仓库路径不一致（仓库可能被重命名/移动过）。" -ForegroundColor Yellow
    Write-Host "    执行 cargo clean 清理旧缓存（避免 tauri_build 读取不存在的旧路径失败）..." -ForegroundColor Yellow
    Invoke-Native cargo @('clean') $SrcTauri
}

# --- 2~4. 切换 → 构建 → 收集；5. finally 保证还原 ---------------------------
$swapped = $false
try {
    Write-Host "==> [2/5] 切换 desktop/pro 为 Pro 实现（物理替换）" -ForegroundColor Cyan
    Rename-Item -Path $ProDir -NewName 'pro-free-backup'
    $swapped = $true
    New-Item -ItemType Directory -Force -Path $ProDir | Out-Null
    Copy-Item (Join-Path $ProRepo 'Cargo.toml') $ProDir
    Copy-Item (Join-Path $ProRepo 'src') $ProDir -Recurse
    if ((Get-ProEdition) -ne 'official') {
        throw "切换后 EDITION 不是 official，私有实现异常，已触发还原。"
    }

    Write-Host "==> [3/5] 物理清除 soundlink-pro 缓存（G10：切换后必清，cargo clean -p 在此场景不可靠）" -ForegroundColor Cyan
    Clear-SoundlinkProCache

    Write-Host "==> [4/5] 前端依赖 + tauri build --features tauri_app --bundles nsis,msi" -ForegroundColor Cyan
    if (-not $SkipUiInstall -and -not (Test-Path (Join-Path $UiDir 'node_modules'))) {
        Invoke-Native npm @('ci') $UiDir
    }
    # tauri-build 2.x 在 build.rs 阶段校验 tauri.conf.json resources 路径存在性。
    # qr_helper.exe 是同 crate 的另一个 bin，cargo 先跑 build.rs 再编译 bin，
    # 因此 tauri build 启动时 qr_helper.exe 还不存在，会报：
    #   resource path `target\release\qr_helper.exe` doesn't exist
    # 创建空占位文件让 build.rs 校验通过，tauri build 内部的 cargo build 会编译
    # 真正的 qr_helper.exe 覆盖它（qr_helper required-features = tauri_app，必被编译）。
    $qrHelper = Join-Path $SrcTauri 'target\release\qr_helper.exe'
    if (-not (Test-Path $qrHelper)) {
        New-Item -ItemType Directory -Force -Path (Split-Path $qrHelper) | Out-Null
        New-Item -ItemType File -Path $qrHelper -Force | Out-Null
        Write-Host "    已创建 qr_helper.exe 占位文件（cargo build 会覆盖为真实产物）" -ForegroundColor DarkGray
    }
    Invoke-Native npm @('exec', '--prefix', '..\ui', 'tauri', '--', 'build', '--features', 'tauri_app', '--bundles', 'nsis,msi') $SrcTauri

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
    Copy-Item $exe (Join-Path $DistDir "SoundLink_${Version}_x64-portable.exe") -Force
} finally {
    if ($swapped) {
        Write-Host "==> 还原仓库为免费版（删除私有副本、恢复免费实现）" -ForegroundColor Cyan
        if (Test-Path $ProDir) { Remove-Item $ProDir -Recurse -Force }
        Rename-Item -Path $BackupDir -NewName 'pro'
        Clear-SoundlinkProCache
        if ((Get-ProEdition) -ne 'community') {
            throw "还原校验失败：desktop/pro EDITION 不是 community，请人工检查 desktop\pro。"
        }
        Write-Host "    已还原：desktop/pro = 免费实现（EDITION=community）" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "Pro 版构建完成（v$Version，EDITION=official），仓库已还原为免费版：" -ForegroundColor Green
Get-ChildItem $DistDir | ForEach-Object { Write-Host "  $($_.FullName)" }
