#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IOS_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(cd "$IOS_DIR/../../.." && pwd)"
OPUS_SRC="$PROJECT_ROOT/mobile/flutter_app/android/app/src/main/cpp/opus"
BUILD_ROOT="$IOS_DIR/build/opus"
OUTPUT_DIR="$IOS_DIR/Frameworks"
OUTPUT_XCFRAMEWORK="$OUTPUT_DIR/Opus.xcframework"

if [ ! -f "$OPUS_SRC/CMakeLists.txt" ]; then
  echo "找不到 libopus 源码：$OPUS_SRC" >&2
  exit 1
fi

rm -rf "$BUILD_ROOT" "$OUTPUT_XCFRAMEWORK"
mkdir -p "$BUILD_ROOT" "$OUTPUT_DIR"

build_opus() {
  local name="$1"
  local sdk="$2"
  local archs="$3"
  local build_dir="$BUILD_ROOT/$name"

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

  if [ ! -f "$lib_path" ]; then
    lib_path="$(find "$BUILD_ROOT/$name" -name libopus.a -type f | head -n 1)"
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
  <string>1.5.2</string>
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

xcodebuild -create-xcframework \
  -framework "$BUILD_ROOT/ios-arm64/Opus.framework" \
  -framework "$BUILD_ROOT/ios-simulator/Opus.framework" \
  -output "$OUTPUT_XCFRAMEWORK"

echo "已生成：$OUTPUT_XCFRAMEWORK"
