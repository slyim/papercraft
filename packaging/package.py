"""Package an already-built native binary; verify every generated archive."""

import argparse
import functools
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import tarfile
import urllib.error
import urllib.request
import zipfile


ROOT = Path(__file__).resolve().parent.parent
NOTICE_NAMES = (
    "LICENSE", "LICENSE-MIT", "LICENSE-APACHE", "LICENSE.md", "LICENSE.txt",
    "LICENSE.chromium", "COPYING", "NOTICE",
)
NOTICE = re.compile(r"(?i)^(licen[sc]e|copying|notice|copyright|authors|unlicense|ofl|ufl)")


def output(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


@functools.cache
def download(url):
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def collect_notices(metadata, destination):
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    seen, pending = set(), [metadata["resolve"]["root"]]
    while pending:
        package_id = pending.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        pending.extend(d["pkg"] for d in nodes[package_id]["deps"]
                       if any(k["kind"] != "dev" for k in d["dep_kinds"]))
    records = []
    for package_id in sorted(seen - {metadata["resolve"]["root"]}):
        package = packages[package_id]
        source = Path(package["manifest_path"]).parent
        target = destination / "crates" / f'{package["name"]}-{package["version"]}'
        target.mkdir(parents=True)
        files, sources = [], []
        has_root_license = False
        for path in source.rglob("*"):
            relative = path.relative_to(source)
            in_notices = any(part.lower() in ("fonts", "licenses") for part in relative.parts[:-1])
            if (path.is_file() and not path.is_symlink()
                    and (NOTICE.match(path.name) or in_notices and path.suffix in (".txt", ".md"))
                    and path.suffix not in (".rs", ".c", ".h")):
                copied = target / relative
                copied.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, copied)
                files.append(relative.as_posix())
                has_root_license |= len(relative.parts) == 1 and path.name.upper().startswith(("LICENSE", "COPYING", "UNLICENSE"))
        if package["name"] == "dispatch" or "github.com/madsmtm/objc2" in (package.get("repository") or ""):
            shutil.copy2(ROOT / "licenses/steven-sheldon-MIT.txt", target)
            files.append("steven-sheldon-MIT.txt")
            sources.append("https://github.com/madsmtm/objc2/blob/4fc083f1c6d6784577e38b0ee8dbd344481e2fd2/LICENSE.txt")
            has_root_license |= package["name"] == "dispatch"
        if not has_root_license:
            repository = package.get("repository") or ""
            match = re.match(r"https?://github.com/([^/]+/[^/]+)", repository)
            vcs_file = source / ".cargo_vcs_info.json"
            revision = (json.loads(vcs_file.read_text())["git"]["sha1"] if vcs_file.exists()
                        else (package.get("source") or "").partition("#")[2])
            if not match or not re.fullmatch(r"[0-9a-f]{40}", revision):
                raise RuntimeError(f'Cannot locate pinned license for {package["name"]}')
            for name in NOTICE_NAMES:
                url = f"https://raw.githubusercontent.com/{match[1].removesuffix('.git')}/{revision}/{name}"
                data = download(url)
                if data:
                    (target / name).write_bytes(data)
                    files.append(name)
                    sources.append(url)
            if not any(name.upper().startswith(("LICENSE", "COPYING", "UNLICENSE")) for name in files):
                raise RuntimeError(f'No license found for {package["name"]}')
        record = {k: package[k] for k in ("name", "version", "license", "repository", "authors")}
        record.update(notice_files=sorted(set(files)), notice_sources=sources)
        (target / "PACKAGE.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
        records.append(record)
    (destination / "DEPENDENCIES.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    rust_docs = Path(output("rustc", "--print", "sysroot")) / "share/doc/rust"
    rust_destination = destination / "rust"
    rust_destination.mkdir()
    shutil.copy2(rust_docs / "COPYRIGHT-library.html", rust_destination)
    shutil.copytree(rust_docs / "licenses", rust_destination / "licenses")
    shutil.copytree(ROOT / "licenses", destination / "project")
    print(f"Included notices for {len(records)} dependencies, Rust libraries, fonts and icons.")


def verify_archive(archive, binary_member, expected_binary):
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as contents:
            names = contents.namelist()
            binary = contents.read(binary_member)
            bad = contents.testzip()
            assert bad is None, f"Corrupt ZIP member: {bad}"
    else:
        with tarfile.open(archive, "r:gz") as contents:
            names = contents.getnames()
            binary = contents.extractfile(binary_member).read()
    assert all(not name.startswith("/") and ".." not in Path(name).parts for name in names)
    assert binary == expected_binary, "Archived executable differs from the release build"
    assert any(name.endswith("/LICENSE-MIT") for name in names)
    assert any(name.endswith("/licenses/DEPENDENCIES.json") for name in names)
    assert any(name.endswith("/licenses/rust/COPYRIGHT-library.html") for name in names)
    print(f"Verified {archive.name}: executable, notices and archive integrity.")


def verify_executable(binary, platform):
    data = binary.read_bytes()
    arm = platform.endswith("arm64")
    if platform.startswith("linux-"):
        assert data[:6] == b"\x7fELF\x02\x01", "Expected a little-endian ELF64 binary"
        assert struct.unpack_from("<H", data, 18)[0] == (183 if arm else 62)
    elif platform.startswith("macos-"):
        assert struct.unpack_from("<II", data) == (0xFEEDFACF, 0x100000C if arm else 0x1000007)
    else:
        assert data[:2] == b"MZ"
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        assert data[pe:pe + 4] == b"PE\0\0"
        assert struct.unpack_from("<H", data, pe + 4)[0] == 0x8664
        assert struct.unpack_from("<H", data, pe + 24 + 68)[0] == 2, "Expected the Windows GUI subsystem"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("linux-x86_64", "linux-arm64", "macos-arm64", "macos-x86_64", "windows-x86_64"))
    parser.add_argument("target")
    args = parser.parse_args()
    assert f"host: {args.target}" in output("rustc", "--version", "--verbose"), "Build on the matching native runner"
    metadata = json.loads(output("cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", args.target))
    package = next(p for p in metadata["packages"] if p["id"] == metadata["resolve"]["root"])
    version = package["version"]
    assert re.fullmatch(r"\d+\.\d+\.\d+", version)
    stem = f"papercraft-{version}-{args.platform}"
    stage = ROOT / "target/release-stage" / stem
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    mac = args.platform.startswith("macos-")
    executable = "papercraft.exe" if args.platform.startswith("windows-") else "papercraft"
    built = ROOT / "target/release" / executable
    binary = stage / ("PaperCraft.app/Contents/MacOS" if mac else "") / executable
    resources = stage / "PaperCraft.app/Contents/Resources" if mac else stage
    binary.parent.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    shutil.copy2(built, binary)
    verify_executable(binary, args.platform)
    for name in ("LICENSE-MIT", "THIRD_PARTY_NOTICES.md"):
        shutil.copy2(ROOT / name, resources)
    collect_notices(metadata, resources / "licenses")
    (stage / "README.txt").write_text(
        f"PaperCraft {version} ({args.platform}) — early preview\n\n"
        + ("Move PaperCraft.app to Applications. This app is ad-hoc signed, not notarized; macOS may require approval in Privacy & Security.\n" if mac
           else "Run papercraft.exe. No Rust or Node installation is needed.\n" if executable.endswith(".exe")
           else "Run ./papercraft from this folder. Requires glibc 2.35+, libxkbcommon-x11, a desktop session and OpenGL/EGL drivers. File dialogs use a desktop portal.\n")
        + "\nUse SHA256SUMS.txt from the release page to verify downloads.\n"
        "Documents with tokens use format version 2; older published builds cannot open them.\n"
        "https://github.com/slyim/papercraft/releases/tag/v" + version + "\n",
        encoding="utf-8",
    )
    if mac:
        with (ROOT / "packaging/macos/Info.plist").open("rb") as source:
            plist = plistlib.load(source)
        assert plist["CFBundleShortVersionString"] == version
        with (stage / "PaperCraft.app/Contents/Info.plist").open("wb") as target:
            plistlib.dump(plist, target)
        shutil.copy2(ROOT / "assets/papercraft.icns", resources)
        app = stage / "PaperCraft.app"
        subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(app)], check=True)
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
    destination = ROOT / "target/release-downloads"
    destination.mkdir(exist_ok=True)
    formats = ("zip", "gztar") if mac else ("zip",) if executable.endswith(".exe") else ("gztar",)
    checksums = []
    for archive_format in formats:
        if archive_format == "zip":
            archive = destination / f"{stem}.zip"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, strict_timestamps=False) as contents:
                for path in stage.rglob("*"):
                    contents.write(path, f"{stem}/{path.relative_to(stage).as_posix()}")
        else:
            archive = Path(shutil.make_archive(str(destination / stem), archive_format, stage.parent, stem))
        verify_archive(archive, f"{stem}/{binary.relative_to(stage).as_posix()}", binary.read_bytes())
        checksums.append(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
    (destination / f"SHA256SUMS-{args.platform}.txt").write_text("".join(checksums), encoding="utf-8")


if __name__ == "__main__":
    main()
