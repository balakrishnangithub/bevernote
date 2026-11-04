"""Create a clean source archive and checksums for locally verified artifacts."""

from __future__ import annotations

import hashlib
from pathlib import Path
import shutil
import zipfile


ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / "release"
SOURCE_PREFIX = "Bevernote-0.1.1-source"
ROOT_FILES = {
    ".gitattributes",
    ".gitignore",
    ".prettierrc.json",
    "Cargo.lock",
    "Cargo.toml",
    "CONTRIBUTING.md",
    "LICENSE",
    "README.md",
    "SECURITY.md",
    "eslint.config.js",
    "index.html",
    "package-lock.json",
    "package.json",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "tsconfig.json",
    "vite.config.ts",
}
SOURCE_DIRECTORIES = (".github", "crates", "docs", "public", "scripts", "src", "src-tauri", "tests")
EXCLUDED_DIRECTORIES = {
    ".git",
    "dist",
    "gen",
    "node_modules",
    "reference",
    "release",
    "target",
    "test-data",
    "test-results",
}
ARTIFACTS = {
    ROOT / "target/release/bundle/nsis/Bevernote_0.1.1_x64-setup.exe": "Bevernote_0.1.1_x64-setup.exe",
    ROOT / "target/release/bundle/msi/Bevernote_0.1.1_x64_en-US.msi": "Bevernote_0.1.1_x64_en-US.msi",
    ROOT / "src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk": "Bevernote_0.1.1_arm64-debug.apk",
}


def source_files() -> list[Path]:
    files = [ROOT / name for name in sorted(ROOT_FILES)]
    for directory in SOURCE_DIRECTORIES:
        files.extend(
            path
            for path in (ROOT / directory).rglob("*")
            if path.is_file()
            and not any(part in EXCLUDED_DIRECTORIES for part in path.relative_to(ROOT).parts)
        )
    for path in files:
        relative = path.relative_to(ROOT)
        if not path.is_file():
            raise FileNotFoundError(path)
        if path.suffix.lower() in {".enex", ".env", ".sqlite", ".db", ".apk", ".exe", ".msi"}:
            raise ValueError(f"Private or binary file cannot enter source archive: {relative}")
        if "credential" in path.name.lower() or "secret" in path.name.lower():
            raise ValueError(f"Potential credential file cannot enter source archive: {relative}")
    return sorted(files)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    RELEASE.mkdir(exist_ok=True)
    files = source_files()
    source_archive = RELEASE / f"{SOURCE_PREFIX}.zip"
    with zipfile.ZipFile(source_archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in files:
            relative = path.relative_to(ROOT).as_posix()
            archive.write(path, f"{SOURCE_PREFIX}/{relative}")
    with zipfile.ZipFile(source_archive) as archive:
        if archive.testzip() is not None:
            raise ValueError("Source archive failed its CRC check")

    apk_source, apk_name = next(item for item in ARTIFACTS.items() if item[1].endswith(".apk"))
    with zipfile.ZipFile(apk_source) as apk:
        if apk.testzip() is not None:
            raise ValueError("APK failed its ZIP integrity check")
        native = apk.getinfo("lib/arm64-v8a/libbevernote_lib.so")
        print(f"APK native library: {native.filename} ({native.file_size:,} bytes uncompressed)")
        print(f"APK entries: {len(apk.infolist()):,}")

    delivered = [source_archive]
    for source, name in ARTIFACTS.items():
        destination = RELEASE / name
        shutil.copy2(source, destination)
        delivered.append(destination)
    checksums = "".join(f"{sha256(path)}  {path.name}\n" for path in delivered)
    (RELEASE / "SHA256SUMS.txt").write_text(checksums, encoding="utf-8")
    print(f"Source archive: {len(files)} files; no private samples or generated build files")
    for path in delivered:
        print(f"{path.name}: {path.stat().st_size:,} bytes")
    print("SHA256SUMS.txt written")


if __name__ == "__main__":
    main()
