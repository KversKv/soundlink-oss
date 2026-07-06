# CMakeLists.txt — 构建 libsoundlink_opus.so（JNI 桥接 + libopus）。
#
# libopus 源码已置于 src/main/cpp/opus/（取自 https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz）。
# 在 app/build.gradle.kts 中启用 externalNativeBuild { cmake { path = ... } }。

cmake_minimum_required(VERSION 3.22.1)
project(soundlink_opus)

# ===== libopus 源码构建（静态库）=====
# 禁用 DRED/OSCE/ambisonics 等非必需 ML 特性，缩短 NDK 构建时间并避免交叉编译兼容问题。
add_subdirectory(opus)

# ===== JNI 桥接 =====
add_library(soundlink_opus SHARED opus_jni.c)

target_link_libraries(soundlink_opus opus log)

target_include_directories(soundlink_opus PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/opus/include)
