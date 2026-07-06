# CMakeLists.txt — 构建 libsoundlink_opus.so（JNI 桥接 + libopus）。
#
# 前置：将 libopus 源码置于 src/main/cpp/opus/（或替换为预编译静态库）。
#       libopus 源码取自 https://gitlab.xiph.org/xiph/opus
# 在 app/build.gradle.kts 中启用 externalNativeBuild { cmake { path = ... } }。

cmake_minimum_required(VERSION 3.22.1)
project(soundlink_opus)

# ===== libopus 源码构建（推荐）=====
# 若已放置 opus 源码到 cpp/opus/，取消下方注释构建静态库。
# add_subdirectory(opus)

# ===== JNI 桥接 =====
add_library(soundlink_opus SHARED opus_jni.c)

# 链接 opus；若用 add_subdirectory(opus)，目标名为 opus。
# target_link_libraries(soundlink_opus opus log)
target_link_libraries(soundlink_opus log)

# 头文件搜索路径（opus 源码构建时）。
# target_include_directories(soundlink_opus PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/opus/include)
