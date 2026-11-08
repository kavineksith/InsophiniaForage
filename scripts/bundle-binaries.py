#!/usr/bin/env python3
"""
Bundle portable binaries into src-tauri/resources/bin/ so they ship inside
the installer. Run BEFORE `npm run tauri:build` to produce an offline-
ready installer.

Usage:
    python3 scripts/bundle-binaries.py --components php mariadb nginx phpmyadmin composer
    python3 scripts/bundle-binaries.py --all           # everything
    python3 scripts/bundle-binaries.py --php-only      # just PHP static
    python3 scripts/bundle-binaries.py --list-versions # show available versions

What it does:
    1. Detects the current OS + arch (linux-x86_64, macos-arm64, etc).
    2. Downloads each requested component from its official source.
    3. Verifies SHA-256 checksums where published.
    4. Extracts into src-tauri/resources/bin/<platform>/<arch>/<component>-<version>/
    5. Reports the total size and what got bundled.

After this script runs, `npm run tauri:build` will pick up the resources
automatically because tauri.conf.json includes the resources/ dir.

To skip the bundled binaries at runtime and prefer system installs, set
the binary_strategy to "prefer_system" in the wizard.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import platform
import shutil
import stat
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

PROJECT_ROOT = Path(__file__).resolve().parent.parent
RESOURCES_DIR = PROJECT_ROOT / "src-tauri" / "resources" / "bin"

# Pinned versions — bump these in sync with src-tauri/src/core/settings.rs
# `Settings::default().versions`.
VERSIONS = {
    "php": "8.3.14",
    "mariadb": "11.4.4",
    "nginx": "1.27.3",
    "phpmyadmin": "5.2.2",
    "composer": "2.8.3",
}

# SHA-256 checksums. `None` means "no published checksum, skip verification"
# (the script will print a warning). For production you should fill these in
# from the official source after each release.
CHECKSUMS: dict[tuple[str, str], Optional[str]] = {
    # Example (fill in before shipping):
    # ("php", "8.3.14"): "abc123...",
    ("php", "8.3.14"): None,
    ("mariadb", "11.4.4"): None,
    ("nginx", "1.27.3"): None,
    ("phpmyadmin", "5.2.2"): None,
    ("composer", "2.8.3"): None,
}

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------

def detect_platform() -> tuple[str, str]:
    """Return (os_token, arch_token) matching Tauri's bundle naming."""
    sys_os = platform.system().lower()
    machine = platform.machine().lower()

    if sys_os == "linux":
        os_token = "linux"
    elif sys_os == "darwin":
        os_token = "macos"
    elif sys_os == "windows":
        os_token = "windows"
    else:
        raise RuntimeError(f"unsupported OS: {sys_os}")

    if machine in ("x86_64", "amd64"):
        arch_token = "x86_64"
    elif machine in ("aarch64", "arm64"):
        arch_token = "aarch64"
    else:
        raise RuntimeError(f"unsupported arch: {machine}")

    return os_token, arch_token


# ---------------------------------------------------------------------------
# Download helpers
# ---------------------------------------------------------------------------

def download(url: str, dest: Path) -> None:
    """Download a URL to a file with a progress bar."""
    print(f"  ↓ {url}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    req = urllib.request.Request(url, headers={"User-Agent": "InsophiniaForage-bundler/0.1"})
    with urllib.request.urlopen(req, timeout=300) as resp:
        total = int(resp.headers.get("Content-Length", 0))
        with open(dest, "wb") as f:
            downloaded = 0
            while True:
                chunk = resp.read(64 * 1024)
                if not chunk:
                    break
                f.write(chunk)
                downloaded += len(chunk)
                if total > 0:
                    pct = downloaded * 100 // total
                    bar = "█" * (pct // 5) + "░" * (20 - pct // 5)
                    print(f"\r  {bar} {pct:3d}% ({downloaded // 1024 // 1024} MB)",
                          end="", flush=True)
            print()
    print(f"  ✓ saved {dest.name} ({dest.stat().st_size // 1024 // 1024} MB)")


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(64 * 1024)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def verify(path: Path, expected: Optional[str], component: str, version: str) -> None:
    if expected is None:
        print(f"  ⚠ no checksum for {component}-{version}, skipping verification")
        return
    actual = sha256(path)
    if actual != expected:
        print(f"  ✗ checksum mismatch for {component}-{version}")
        print(f"    expected: {expected}")
        print(f"    actual:   {actual}")
        sys.exit(1)
    print(f"  ✓ checksum verified")


# ---------------------------------------------------------------------------
# Extraction helpers
# ---------------------------------------------------------------------------

def extract_zip(archive: Path, dest: Path, strip_top: bool = False) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as zf:
        for member in zf.infolist():
            name = member.filename
            if strip_top:
                # Strip the first path component.
                parts = name.split("/", 1)
                if len(parts) < 2 or not parts[1]:
                    continue
                name = parts[1]
            target = dest / name
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with zf.open(member) as src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst)


def extract_targz(archive: Path, dest: Path, strip_top: bool = False) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "r:gz") as tf:
        for member in tf.getmembers():
            name = member.name
            if strip_top:
                parts = name.split("/", 1)
                if len(parts) < 2 or not parts[1]:
                    continue
                name = parts[1]
            target = dest / name
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                src = tf.extractfile(member)
                if src is None:
                    continue
                with src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst)
                # Preserve permissions.
                mode = member.mode or 0o755
                os.chmod(target, mode)


def make_executable(path: Path) -> None:
    """Set executable bit on every file in a directory tree (POSIX only)."""
    if os.name != "posix":
        return
    for root, _, files in os.walk(path):
        for f in files:
            p = Path(root) / f
            # Heuristic: make it executable if it has no extension or
            # ends with .sh. PHP/JS binaries get +x too.
            if p.suffix in ("", ".sh", ".phar"):
                cur = p.stat().st_mode
                os.chmod(p, cur | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


# ---------------------------------------------------------------------------
# Per-component bundlers
# ---------------------------------------------------------------------------

def bundle_php(cache: Path, dest: Path, os_token: str, arch_token: str, version: str) -> None:
    """Bundle a static PHP build from crazywhalecc/static-php-cli."""
    if dest.exists():
        print(f"  ✓ php-{version} already bundled at {dest}")
        return

    # Map (os, arch) to the asset name pattern used by static-php-cli.
    # See https://github.com/crazywhalecc/static-php-cli/releases
    os_arch_token = f"{os_token}-{arch_token}"
    # static-php-cli uses different naming; we use a simplified version.
    # Real-world: you'd query the GitHub releases API for the exact asset
    # name. For this script we use a known pattern.
    if os_token == "windows":
        url = f"https://github.com/crazywhalecc/static-php-cli/releases/download/{version}/php-{version}-static-windows-x86_64.zip"
        archive = cache / f"php-{version}-{os_arch_token}.zip"
        download(url, archive)
        verify(archive, CHECKSUMS.get(("php", version)), "php", version)
        extract_zip(archive, dest, strip_top=False)
    else:
        ext = "tar.gz"
        url = f"https://github.com/crazywhalecc/static-php-cli/releases/download/{version}/php-{version}-static-{os_arch_token}.{ext}"
        archive = cache / f"php-{version}-{os_arch_token}.{ext}"
        download(url, archive)
        verify(archive, CHECKSUMS.get(("php", version)), "php", version)
        extract_targz(archive, dest, strip_top=False)
    make_executable(dest)


def bundle_mariadb(cache: Path, dest: Path, os_token: str, arch_token: str, version: str) -> None:
    if dest.exists():
        print(f"  ✓ mariadb-{version} already bundled at {dest}")
        return

    # Map to MariaDB's binary tarball naming.
    if os_token == "linux" and arch_token == "x86_64":
        os_arch = "linux-systemd-x86_64"
    elif os_token == "linux" and arch_token == "aarch64":
        os_arch = "linux-systemd-aarch64"
    elif os_token == "macos" and arch_token == "x86_64":
        os_arch = "darwin-x86_64"
    elif os_token == "macos" and arch_token == "aarch64":
        os_arch = "darwin-arm64"
    elif os_token == "windows" and arch_token == "x86_64":
        os_arch = "winx64"
    else:
        raise RuntimeError(f"no MariaDB build for {os_token}-{arch_token}")

    if os_token == "windows":
        url = f"https://archive.mariadb.org/mariadb-{version}/bintar-{os_arch}-mariadb-{version}.zip"
        archive = cache / f"mariadb-{version}-{os_arch}.zip"
        download(url, archive)
        verify(archive, CHECKSUMS.get(("mariadb", version)), "mariadb", version)
        extract_zip(archive, dest, strip_top=True)
    else:
        url = f"https://archive.mariadb.org/mariadb-{version}/bintar-{os_arch}-mariadb-{version}.tar.gz"
        archive = cache / f"mariadb-{version}-{os_arch}.tar.gz"
        download(url, archive)
        verify(archive, CHECKSUMS.get(("mariadb", version)), "mariadb", version)
        extract_targz(archive, dest, strip_top=True)
    make_executable(dest)


def bundle_nginx(cache: Path, dest: Path, os_token: str, arch_token: str, version: str) -> None:
    """Bundle nginx. On Linux/macOS we use the official source-based builds
    from nginx.org; on Windows we use the official Windows binary."""
    if dest.exists():
        print(f"  ✓ nginx-{version} already bundled at {dest}")
        return

    if os_token == "windows":
        url = f"https://nginx.org/download/nginx-{version}.zip"
        archive = cache / f"nginx-{version}.zip"
        download(url, archive)
        verify(archive, CHECKSUMS.get(("nginx", version)), "nginx", version)
        extract_zip(archive, dest, strip_top=True)
        make_executable(dest)
    else:
        # On Linux/macOS there's no official portable binary. We'd need
        # to compile from source. For now, print a warning and skip —
        # the user should rely on system nginx via package manager.
        print(f"  ⚠ nginx bundling not supported on {os_token}.")
        print(f"    Install via your package manager: sudo apt install nginx")
        print(f"    The app will use the system nginx at runtime.")


def bundle_phpmyadmin(cache: Path, dest: Path, version: str) -> None:
    if dest.exists():
        print(f"  ✓ phpmyadmin-{version} already bundled at {dest}")
        return
    url = f"https://files.phpmyadmin.net/phpMyAdmin/{version}/phpMyAdmin-{version}-all-languages.zip"
    archive = cache / f"phpmyadmin-{version}.zip"
    download(url, archive)
    verify(archive, CHECKSUMS.get(("phpmyadmin", version)), "phpmyadmin", version)
    extract_zip(archive, dest, strip_top=True)


def bundle_composer(cache: Path, dest: Path, version: str) -> None:
    """Composer is a single .phar file. We download the version-pinned
    phar from getcomposer.org."""
    if dest.exists():
        print(f"  ✓ composer-{version} already bundled at {dest}")
        return
    dest.mkdir(parents=True, exist_ok=True)
    url = f"https://github.com/composer/composer/releases/download/{version}/composer.phar"
    archive = dest / "composer.phar"
    download(url, archive)
    verify(archive, CHECKSUMS.get(("composer", version)), "composer", version)
    make_executable(dest)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def dir_size_mb(path: Path) -> int:
    total = 0
    for root, _, files in os.walk(path):
        for f in files:
            total += os.path.getsize(os.path.join(root, f))
    return total // 1024 // 1024


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--components", nargs="*",
                        default=["php", "mariadb", "phpmyadmin", "composer"],
                        choices=["php", "mariadb", "nginx", "phpmyadmin", "composer"],
                        help="which components to bundle")
    parser.add_argument("--all", action="store_true",
                        help="bundle all components (including nginx)")
    parser.add_argument("--php-only", action="store_true",
                        help="shortcut for --components php")
    parser.add_argument("--list-versions", action="store_true",
                        help="print the pinned versions and exit")
    parser.add_argument("--clean", action="store_true",
                        help="remove the resources dir before bundling")
    args = parser.parse_args()

    if args.list_versions:
        for k, v in VERSIONS.items():
            print(f"  {k:12s} {v}")
        return 0

    if args.all:
        components = ["php", "mariadb", "nginx", "phpmyadmin", "composer"]
    elif args.php_only:
        components = ["php"]
    else:
        components = args.components

    os_token, arch_token = detect_platform()
    print(f"\nPlatform: {os_token}-{arch_token}")
    print(f"Components: {', '.join(components)}")
    print(f"Output: {RESOURCES_DIR}/{os_token}-{arch_token}/\n")

    if args.clean and RESOURCES_DIR.exists():
        print(f"cleaning {RESOURCES_DIR}")
        shutil.rmtree(RESOURCES_DIR)

    cache = PROJECT_ROOT / "target" / "bundle-cache"
    cache.mkdir(parents=True, exist_ok=True)

    platform_dir = RESOURCES_DIR / f"{os_token}-{arch_token}"
    platform_dir.mkdir(parents=True, exist_ok=True)

    for comp in components:
        version = VERSIONS[comp]
        dest = platform_dir / f"{comp}-{version}"
        print(f"\n[{comp} {version}]")
        try:
            if comp == "php":
                bundle_php(cache, dest, os_token, arch_token, version)
            elif comp == "mariadb":
                bundle_mariadb(cache, dest, os_token, arch_token, version)
            elif comp == "nginx":
                bundle_nginx(cache, dest, os_token, arch_token, version)
            elif comp == "phpmyadmin":
                bundle_phpmyadmin(cache, dest, version)
            elif comp == "composer":
                bundle_composer(cache, dest, version)
        except Exception as e:
            print(f"  ✗ failed: {e}")
            # Continue with other components — partial bundle is still useful.

    # Summary
    print("\n" + "=" * 60)
    print("Bundle summary:")
    print(f"  Location: {platform_dir}")
    print(f"  Total size: {dir_size_mb(platform_dir)} MB")
    print("\nNext steps:")
    print("  1. Verify resources are picked up by tauri.conf.json")
    print("  2. Run: npm run tauri:build")
    print("  3. The installer will be at src-tauri/target/release/bundle/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
