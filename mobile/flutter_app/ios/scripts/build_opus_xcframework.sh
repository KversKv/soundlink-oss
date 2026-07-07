#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FLUTTER_APP_DIR="$(cd "$IOS_DIR/.." && pwd)"

OPUS_VERSION="1.5.2"
OPUS_ARCHIVE="opus-$OPUS_VERSION.tar.gz"
OPUS_URL="https://downloads.xiph.org/releases/opus/$OPUS_ARCHIVE"

# 最终源码目录：Android 和 iOS 共用这一份 Opus 源码
OPUS_SRC="$FLUTTER_APP_DIR/android/app/src/main/cpp/opus"

# 下载缓存目录
OPUS_CACHE_DIR="$FLUTTER_APP_DIR/.third_party"
OPUS_CACHE_ARCHIVE="$OPUS_CACHE_DIR/$OPUS_ARCHIVE"
OPUS_CACHE_SRC="$OPUS_CACHE_DIR/opus-$OPUS_VERSION"

# 如果你之前手动下载到了 ios/opus-1.5.2.tar.gz，这里会优先复用它
IOS_LOCAL_ARCHIVE="$IOS_DIR/$OPUS_ARCHIVE"

BUILD_ROOT="$IOS_DIR/build/opus"
OUTPUT_DIR="$IOS_DIR/Frameworks"
OUTPUT_XCFRAMEWORK="$OUTPUT_DIR/Opus.xcframework"

# 可选：如果你想强制脚本走代理，可以这样执行：
# OPUS_DOWNLOAD_PROXY=http://127.0.0.1:7897 bash scripts/build_opus_xcframework.sh
OPUS_DOWNLOAD_PROXY="${OPUS_DOWNLOAD_PROXY:-}"

log() {
  echo "[build-opus] $*"
}

die() {
  echo "[build-opus] ERROR: $*" >&2
  exit 1
}

validate_archive() {
  local archive="$1"

  if [ ! -f "$archive" ]; then
    return 1
  fi

  log "校验压缩包：$archive"

  if ! gzip -t "$archive" >/dev/null 2>&1; then
    log "gzip 校验失败：$archive"
    return 1
  fi

  if ! tar -tzf "$archive" >/dev/null 2>&1; then
    log "tar 内容校验失败：$archive"
    return 1
  fi

  return 0
}

download_archive() {
  mkdir -p "$OPUS_CACHE_DIR"

  local tmp_archive="$OPUS_CACHE_ARCHIVE.tmp"

  log "下载 Opus $OPUS_VERSION"
  log "URL: $OPUS_URL"
  log "目标: $OPUS_CACHE_ARCHIVE"

  rm -f "$tmp_archive"

  local curl_args=(
    -L
    --fail
    --retry 5
    --retry-delay 2
    --connect-timeout 30
    -o "$tmp_archive"
    "$OPUS_URL"
  )

  if [ -n "$OPUS_DOWNLOAD_PROXY" ]; then
    log "使用代理：$OPUS_DOWNLOAD_PROXY"
    curl_args=(-x "$OPUS_DOWNLOAD_PROXY" "${curl_args[@]}")
  fi

  if command -v curl >/dev/null 2>&1; then
    curl "${curl_args[@]}"
  else
    die "找不到 curl，无法下载 libopus：$OPUS_URL"
  fi

  if ! validate_archive "$tmp_archive"; then
    rm -f "$tmp_archive"
    die "下载的 Opus 压缩包校验失败，请检查网络或代理"
  fi

  mv "$tmp_archive" "$OPUS_CACHE_ARCHIVE"
  log "下载完成：$OPUS_CACHE_ARCHIVE"
}

prepare_archive() {
  mkdir -p "$OPUS_CACHE_DIR"

  # 1. 如果缓存包有效，直接使用
  if validate_archive "$OPUS_CACHE_ARCHIVE"; then
    log "使用已有缓存包：$OPUS_CACHE_ARCHIVE"
    return
  fi

  # 2. 缓存包存在但损坏，删除
  if [ -f "$OPUS_CACHE_ARCHIVE" ]; then
    log "删除损坏的缓存包：$OPUS_CACHE_ARCHIVE"
    rm -f "$OPUS_CACHE_ARCHIVE"
  fi

  # 3. 如果 ios 目录下有你手动下载的有效包，复制到缓存目录
  if validate_archive "$IOS_LOCAL_ARCHIVE"; then
    log "发现 ios 目录下已有有效压缩包，复制到缓存目录"
    cp "$IOS_LOCAL_ARCHIVE" "$OPUS_CACHE_ARCHIVE"
    return
  fi

  # 4. 否则重新下载
  download_archive
}

prepare_opus_source() {
  if [ -f "$OPUS_SRC/CMakeLists.txt" ]; then
    log "Opus 源码已存在：$OPUS_SRC"
    return
  fi

  mkdir -p "$(dirname "$OPUS_SRC")" "$OPUS_CACHE_DIR"

  prepare_archive

  log "解压 Opus 源码"

  rm -rf "$OPUS_CACHE_SRC"

  if ! tar -xzf "$OPUS_CACHE_ARCHIVE" -C "$OPUS_CACHE_DIR"; then
    log "解压失败，删除疑似损坏的缓存包：$OPUS_CACHE_ARCHIVE"
    rm -f "$OPUS_CACHE_ARCHIVE"
    die "解压 Opus 失败，请重新运行脚本"
  fi

  if [ ! -d "$OPUS_CACHE_SRC" ]; then
    die "解压后未找到源码目录：$OPUS_CACHE_SRC"
  fi

  rm -rf "$OPUS_SRC"
  mv "$OPUS_CACHE_SRC" "$OPUS_SRC"

  log "Opus 源码已准备好：$OPUS_SRC"
}

clean_all() {
  log "清理构建产物"

  rm -rf "$BUILD_ROOT"
  rm -rf "$OUTPUT_XCFRAMEWORK"

  # clean 时也清理源码目录，避免旧源码残留
  rm -rf "$OPUS_SRC"

  # 清理解压缓存，但保留有效 tar.gz，避免每次都重新下载
  rm -rf "$OPUS_CACHE_SRC"

  log "清理完成"
}

if [ "${1:-}" = "clean" ]; then
  clean_all
fi

prepare_opus_source

if [ ! -f "$OPUS_SRC/CMakeLists.txt" ]; then
  echo "找不到 libopus 源码：$OPUS_SRC" >&2
  echo "请确认网络可访问 $OPUS_URL，或手动解压 opus-$OPUS_VERSION 到上述目录。" >&2
  exit 1
fi

rm -rf "$BUILD_ROOT" "$OUTPUT_XCFRAMEWORK"
mkdir -p "$BUILD_ROOT" "$OUTPUT_DIR"

build_opus() {
  local name="$1"
  local sdk="$2"
  local archs="$3"
  local build_dir="$BUILD_ROOT/$name"

  log "开始构建：$name, sdk=$sdk, archs=$archs"

  cmake -S "$OPUS_SRC" -B "$build_dir" -G Xcode \
    -DCMAKE_SYSTEM_NAME=iOS \
    -DCMAKE_OSX_SYSROOT="$sdk" \
    -DCMAKE_OSX_ARCHITECTURES="$archs" \
    -DCMAKE_XCODE_ATTRIBUTE_ONLY_ACTIVE_ARCH=NO \
    -DCMAKE_XCODE_ATTRIBUTE_SKIP_INSTALL=NO \
    -DCMAKE_IOS_INSTALL_COMBINED=NO \
    -DOPUS_INSTALL_PKG_CONFIG_MODULE=OFF \
    -DOPUS_INSTALL_CMAKE_CONFIG_MODULE=OFF \
    -DOPUS_BUILD_PROGRAMS=OFF \
    -DOPUS_BUILD_TESTING=OFF \
    -DOPUS_BUILD_SHARED_LIBRARY=OFF \
    -DOPUS_BUILD_FRAMEWORK=OFF \
    -DOPUS_DISABLE_INTRINSICS=ON \
    -DOPUS_X86_MAY_HAVE_SSE=OFF \
    -DOPUS_X86_MAY_HAVE_SSE2=OFF \
    -DOPUS_X86_MAY_HAVE_SSE4_1=OFF \
    -DOPUS_X86_MAY_HAVE_AVX2=OFF \
    -DOPUS_X86_PRESUME_SSE=OFF \
    -DOPUS_X86_PRESUME_SSE2=OFF \
    -DOPUS_X86_PRESUME_SSE4_1=OFF \
    -DOPUS_X86_PRESUME_AVX2=OFF

  cmake --build "$build_dir" --config Release --target opus
}

make_framework() {
  local name="$1"
  local platform_name="$2"
  local min_os="$3"
  local framework_dir="$BUILD_ROOT/$name/Opus.framework"
  local lib_path="$BUILD_ROOT/$name/Release-$platform_name/libopus.a"

  log "生成 Framework：$name"

  if [ ! -f "$lib_path" ]; then
    lib_path="$(find "$BUILD_ROOT/$name" -name libopus.a -type f | head -n 1 || true)"
  fi

  if [ -z "$lib_path" ] || [ ! -f "$lib_path" ]; then
    echo "未找到 libopus.a：$BUILD_ROOT/$name" >&2
    exit 1
  fi

  mkdir -p "$framework_dir/Headers/opus" "$framework_dir/Modules"

  cp "$lib_path" "$framework_dir/Opus"

  cp "$OPUS_SRC/include/opus.h" "$framework_dir/Headers/opus/opus.h"
  cp "$OPUS_SRC/include/opus_defines.h" "$framework_dir/Headers/opus/opus_defines.h"
  cp "$OPUS_SRC/include/opus_types.h" "$framework_dir/Headers/opus/opus_types.h"
  cp "$OPUS_SRC/include/opus_multistream.h" "$framework_dir/Headers/opus/opus_multistream.h"
  cp "$OPUS_SRC/include/opus_projection.h" "$framework_dir/Headers/opus/opus_projection.h"
  cp "$OPUS_SRC/include/opus_custom.h" "$framework_dir/Headers/opus/opus_custom.h"

  cat > "$framework_dir/Headers/Opus.h" <<'EOF'
#import <opus/opus.h>
#import <opus/opus_defines.h>
#import <opus/opus_types.h>
#import <opus/opus_multistream.h>
#import <opus/opus_projection.h>
#import <opus/opus_custom.h>
EOF

  cat > "$framework_dir/Modules/module.modulemap" <<'EOF'
framework module Opus {
  umbrella header "Opus.h"
  export *
  module * { export * }
}
EOF

  cat > "$framework_dir/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>Opus</string>
  <key>CFBundleIdentifier</key>
  <string>org.xiph.opus</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Opus</string>
  <key>CFBundlePackageType</key>
  <string>FMWK</string>
  <key>CFBundleShortVersionString</key>
  <string>$OPUS_VERSION</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>MinimumOSVersion</key>
  <string>$min_os</string>
</dict>
</plist>
EOF
}

build_opus ios-arm64 iphoneos arm64
build_opus ios-simulator iphonesimulator "arm64;x86_64"

make_framework ios-arm64 iphoneos 13.0
make_framework ios-simulator iphonesimulator 13.0

log "创建 XCFramework"

xcodebuild -create-xcframework \
  -framework "$BUILD_ROOT/ios-arm64/Opus.framework" \
  -framework "$BUILD_ROOT/ios-simulator/Opus.framework" \
  -output "$OUTPUT_XCFRAMEWORK"

log "已生成：$OUTPUT_XCFRAMEWORK"