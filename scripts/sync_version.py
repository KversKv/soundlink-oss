#!/usr/bin/env python3
"""SoundLink 版本同步脚本（version-management V2）。

单一来源：仓库根 `VERSION`（一行纯 SemVer）。
同步目标：
  - desktop/src-tauri/Cargo.toml            [package] version
  - desktop/src-tauri/tauri.conf.json       顶层 version
  - desktop/ui/package.json                 顶层 version
  - mobile/flutter_app/pubspec.yaml         version: <core>+<build_number>

移动端转换规则：去掉预发布后缀，附加 BUILD_NUMBER（默认 major*10000+minor*100+patch）。
website/package.json 不参与同步（独立站点）。

用法：
  python scripts/sync_version.py                  # 写入
  python scripts/sync_version.py --check          # 只校验，不一致则非零退出
  python scripts/sync_version.py --build-number 7 # 指定移动端构建号

详见 docs/NewFunctions/version-management/00-version-management-plan.md。
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VERSION_FILE = REPO_ROOT / "VERSION"

TARGETS = {
    "cargo": REPO_ROOT / "desktop" / "src-tauri" / "Cargo.toml",
    "tauri": REPO_ROOT / "desktop" / "src-tauri" / "tauri.conf.json",
    "ui_pkg": REPO_ROOT / "desktop" / "ui" / "package.json",
    "pubspec": REPO_ROOT / "mobile" / "flutter_app" / "pubspec.yaml",
}

SEMVER_RE = re.compile(
    r"^(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)"
    r"(?:-(?P<pre>[0-9A-Za-z.-]+))?(?:\+(?P<build>[0-9A-Za-z.-]+))?$"
)


def parse_version(raw: str) -> dict:
    text = raw.strip()
    m = SEMVER_RE.match(text)
    if not m:
        raise SystemExit(f"[sync_version] VERSION 内容不是合法 SemVer: {text!r}")
    return m.groupdict()


def read_repo_version() -> dict:
    if not VERSION_FILE.exists():
        raise SystemExit(f"[sync_version] 未找到 VERSION 文件: {VERSION_FILE}")
    return parse_version(VERSION_FILE.read_text(encoding="utf-8"))


def format_product_version(v: dict) -> str:
    core = f'{v["major"]}.{v["minor"]}.{v["patch"]}'
    if v["pre"]:
        return f"{core}-{v['pre']}"
    return core


def default_build_number(v: dict) -> int:
    return int(v["major"]) * 10000 + int(v["minor"]) * 100 + int(v["patch"])


# ---------- 读取当前值 ----------

def parse_cargo_version(text: str) -> str:
    data = tomllib.loads(text)
    return data["package"]["version"]


def parse_json_version(text: str) -> str:
    return json.loads(text)["version"]


def parse_pubspec_version(text: str) -> tuple[str, str | None]:
    m = re.search(r"^version\s*:\s*(\S+)\s*$", text, re.MULTILINE)
    if not m:
        raise SystemExit("[sync_version] pubspec.yaml 未找到顶层 version 字段")
    raw = m.group(1)
    if "+" in raw:
        core, _, bn = raw.partition("+")
        return core, bn
    return raw, None


# ---------- 行级替换（保留原文件格式与注释） ----------

def replace_cargo_version(content: str, new_version: str) -> str:
    """替换 Cargo.toml [package] 段内首个 version = \"...\" 行，保留行尾。"""
    lines = content.splitlines(keepends=True)
    in_package = False
    replaced = False
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_package = stripped == "[package]"
            continue
        if in_package and not replaced:
            # search 仅匹配 `version = "..."` 片段，行尾换行符（含 \r\n）原样保留。
            m = re.search(r'(\s*version\s*=\s*")(.*?)(")', line)
            if m:
                lines[i] = (
                    line[: m.start()]
                    + f'{m.group(1)}{new_version}{m.group(3)}'
                    + line[m.end() :]
                )
                replaced = True
    if not replaced:
        raise SystemExit("[sync_version] Cargo.toml [package] 段内未找到 version 字段")
    return "".join(lines)


def replace_json_top_level_version(content: str, new_version: str, label: str) -> str:
    """替换 JSON 顶层 \"version\" 字段值（仅替换首次匹配）。"""
    pattern = re.compile(r'^(\s*"version"\s*:\s*")(.*?)(")', re.MULTILINE)
    new_content, n = pattern.subn(
        lambda m: f'{m.group(1)}{new_version}{m.group(3)}', content, count=1
    )
    if n != 1:
        raise SystemExit(f"[sync_version] {label} 未找到顶层 version 字段")
    return new_content


def replace_pubspec_version(content: str, core_version: str, build_number: int) -> str:
    """替换 pubspec.yaml 顶层 version: 行 → `<core>+<build_number>`。

    行尾用 `[ \\t]*$` 仅匹配行内空白，避免 \\s*$ 跨行吞掉换行符（曾导致空行丢失）。
    """
    pattern = re.compile(r'^(\s*version\s*:\s*)(\S+)[ \t]*$', re.MULTILINE)
    new_content, n = pattern.subn(
        lambda m: f'{m.group(1)}{core_version}+{build_number}', content, count=1
    )
    if n != 1:
        raise SystemExit("[sync_version] pubspec.yaml 未找到顶层 version 字段")
    return new_content


# ---------- 主流程 ----------

def sync(write: bool, build_number: int | None) -> int:
    v = read_repo_version()
    product_version = format_product_version(v)
    core_version = f'{v["major"]}.{v["minor"]}.{v["patch"]}'
    bn = build_number if build_number is not None else default_build_number(v)

    print(f"[sync_version] VERSION = {product_version}  build_number = {bn}")

    cargo_text = TARGETS["cargo"].read_text(encoding="utf-8")
    tauri_text = TARGETS["tauri"].read_text(encoding="utf-8")
    ui_text = TARGETS["ui_pkg"].read_text(encoding="utf-8")
    pub_text = TARGETS["pubspec"].read_text(encoding="utf-8")

    cargo_now = parse_cargo_version(cargo_text)
    tauri_now = parse_json_version(tauri_text)
    ui_now = parse_json_version(ui_text)
    pub_core_now, pub_bn_now = parse_pubspec_version(pub_text)

    discrepancies = []

    def check(label: str, current: str, expected: str) -> None:
        if current != expected:
            discrepancies.append(f"  {label}: {current!r} → {expected!r}")

    check("Cargo.toml [package].version", cargo_now, product_version)
    check("tauri.conf.json version", tauri_now, product_version)
    check("desktop/ui/package.json version", ui_now, product_version)
    # pubspec: versionName 必须 = core 三段（预发布后缀丢弃，见 plan §3.3）。
    # build_number 不在 check 范围内（允许 CI 单独覆盖为 run_number）。
    check("pubspec.yaml versionName", pub_core_now, core_version)

    if write:
        TARGETS["cargo"].write_text(
            replace_cargo_version(cargo_text, product_version), encoding="utf-8"
        )
        TARGETS["tauri"].write_text(
            replace_json_top_level_version(tauri_text, product_version, "tauri.conf.json"),
            encoding="utf-8",
        )
        TARGETS["ui_pkg"].write_text(
            replace_json_top_level_version(ui_text, product_version, "desktop/ui/package.json"),
            encoding="utf-8",
        )
        # pubspec: 仅写 core 三段 + build_number，不带预发布后缀。
        TARGETS["pubspec"].write_text(
            replace_pubspec_version(pub_text, core_version, bn), encoding="utf-8"
        )
        print(f"[sync_version] 已写入 4 个目标文件（pubspec build_number={bn}）。")

    if discrepancies:
        print("[sync_version] 检测到不一致:")
        for d in discrepancies:
            print(d)
        if not write:
            print("[sync_version] --check 模式：未写入；CI 门应失败。")
            return 1
        print("[sync_version] 已修正以上不一致。")
    else:
        print("[sync_version] 全部目标与 VERSION 一致。")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description="SoundLink 版本同步脚本")
    p.add_argument(
        "--check",
        action="store_true",
        help="只校验，不写入；不一致时非零退出（供 CI 门使用）",
    )
    p.add_argument(
        "--build-number",
        type=int,
        default=None,
        help="移动端 BUILD_NUMBER（默认 major*10000+minor*100+patch）",
    )
    args = p.parse_args()
    return sync(write=not args.check, build_number=args.build_number)


if __name__ == "__main__":
    sys.exit(main())
