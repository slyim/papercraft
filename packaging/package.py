"""Package a native binary; verify archives and real platform installers."""

import argparse
import ctypes
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
import tempfile
import urllib.error
import urllib.request
import zipfile
import uuid


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


def run_native(command, timeout=120, **kwargs):
    """Keep installer tooling and smoke processes bounded; never ignore failure."""
    return subprocess.run(command, check=True, timeout=timeout, cwd=ROOT, **kwargs)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def verify_tree(expected, actual, extra_files=()):
    """Compare all shipped files, not just the executable or notice markers."""
    expected_files = {p.relative_to(expected) for p in expected.rglob("*") if p.is_file()}
    actual_files = {p.relative_to(actual) for p in actual.rglob("*") if p.is_file()}
    assert actual_files == expected_files | {Path(p) for p in extra_files}, "Installed file inventory differs"
    for relative in expected_files:
        installed = actual / relative
        assert not installed.is_symlink(), f"Unexpected installed symlink: {relative}"
        assert digest(expected / relative) == digest(installed), f"Installed bytes differ: {relative}"


def package_dmg(stage, destination, stem, version, work):
    image = destination / f"{stem}.dmg"
    source = work / "dmg-source"
    shutil.copytree(stage, source)
    (source / "Applications").symlink_to("/Applications", target_is_directory=True)
    run_native(["hdiutil", "create", "-ov", "-format", "UDZO", "-fs", "HFS+",
                "-volname", f"PaperCraft {version}", "-srcfolder", str(source), str(image)])
    run_native(["hdiutil", "verify", str(image)])
    mount = work / "mounted"
    mount.mkdir()
    try:
        result = run_native(["hdiutil", "attach", "-readonly", "-nobrowse", "-noautoopen",
                             "-mountpoint", str(mount), "-plist", str(image)], stdout=subprocess.PIPE)
        entities = plistlib.loads(result.stdout)["system-entities"]
        assert any(Path(e["mount-point"]).resolve() == mount.resolve()
                   for e in entities if "mount-point" in e), "DMG was not mounted"
        assert (mount / "Applications").is_symlink()
        assert (mount / "Applications").readlink() == Path("/Applications")
        verify_tree(stage / "PaperCraft.app", mount / "PaperCraft.app")
        assert digest(stage / "README.txt") == digest(mount / "README.txt")
        binary = mount / "PaperCraft.app/Contents/MacOS/papercraft"
        assert binary.stat().st_mode & 0o111, "DMG executable lost execute permission"
        run_native(["codesign", "--verify", "--deep", "--strict", str(mount / "PaperCraft.app")])
    finally:
        if mount.is_mount():
            run_native(["hdiutil", "detach", str(mount)])
    print(f"Verified {image.name}: read-only mount, signature, executable, notices; detached.")
    return image


LINUX_DEPENDS = (
    "libc6 (>= 2.35), libgcc-s1, libdbus-1-3, libx11-6, libx11-xcb1, libxcb1, "
    "libxcursor1, libxi6, libxrandr2, libxkbcommon0, libxkbcommon-x11-0, "
    "libwayland-client0, libwayland-cursor0, libwayland-egl1, libegl1, libgl1"
)


def package_deb(stage, destination, stem, version, platform, work):
    architecture = "arm64" if platform.endswith("arm64") else "amd64"
    root = work / "deb-root"
    executable = root / "usr/bin/papercraft"
    executable.parent.mkdir(parents=True)
    shutil.copy2(stage / "papercraft", executable)
    executable.chmod(0o755)
    resources = root / "usr/share/papercraft"
    resources.mkdir(parents=True)
    for source in stage.iterdir():
        if source.name == "papercraft":
            continue
        if source.is_dir():
            shutil.copytree(source, resources / source.name)
        else:
            shutil.copy2(source, resources / source.name)
    desktop = root / "usr/share/applications/papercraft.desktop"
    desktop.parent.mkdir(parents=True)
    shutil.copy2(ROOT / "packaging/linux/papercraft.desktop", desktop)
    icon = root / "usr/share/icons/hicolor/256x256/apps/papercraft.png"
    icon.parent.mkdir(parents=True)
    shutil.copy2(ROOT / "assets/papercraft-icon.png", icon)
    copyright_file = root / "usr/share/doc/papercraft/copyright"
    copyright_file.parent.mkdir(parents=True)
    shutil.copy2(stage / "LICENSE-MIT", copyright_file)
    control = root / "DEBIAN/control"
    control.parent.mkdir()
    installed_size = (sum(p.stat().st_size for p in root.rglob("*") if p.is_file()) + 1023) // 1024
    fields = {
        "Package": "papercraft", "Version": version, "Section": "graphics",
        "Priority": "optional", "Architecture": architecture,
        "Maintainer": "PaperCraft contributors <noreply@github.com>",
        "Installed-Size": str(installed_size), "Depends": LINUX_DEPENDS,
        "Recommends": "xdg-desktop-portal",
        "Homepage": "https://github.com/slyim/papercraft",
        "Description": "A local design canvas built in Rust and egui.",
    }
    control.write_text("".join(f"{key}: {value}\n" for key, value in fields.items()), encoding="utf-8")
    for path in root.rglob("*"):
        path.chmod(0o755 if path.is_dir() or path == executable else 0o644)
    archive = destination / f"{stem}.deb"
    run_native(["dpkg-deb", "--root-owner-group", "--build", str(root), str(archive)])
    for key, value in fields.items():
        actual = run_native(["dpkg-deb", "--field", str(archive), key],
                            stdout=subprocess.PIPE, text=True).stdout.strip()
        assert actual == value, f"Debian metadata differs: {key}"
    listing = run_native(["dpkg-deb", "--contents", str(archive)],
                         stdout=subprocess.PIPE, text=True).stdout
    assert all(line.split()[1] == "root/root" for line in listing.splitlines()), "Debian ownership differs"
    extracted = work / "deb-extracted"
    run_native(["dpkg-deb", "--extract", str(archive), str(extracted)])
    expected_files = {p.relative_to(root) for p in root.rglob("*")
                      if p.is_file() and "DEBIAN" not in p.relative_to(root).parts}
    assert {p.relative_to(extracted) for p in extracted.rglob("*") if p.is_file()} == expected_files
    for relative in expected_files:
        assert not (extracted / relative).is_symlink()
        assert digest(root / relative) == digest(extracted / relative), f"Debian bytes differ: {relative}"
    assert digest(stage / "papercraft") == digest(extracted / "usr/bin/papercraft")
    assert (extracted / "usr/bin/papercraft").stat().st_mode & 0o111
    verify_tree(resources, extracted / "usr/share/papercraft")
    print(f"Verified {archive.name}: architecture, dependencies, ownership, paths, executable and notices.")
    return archive


def nsis_literal(value):
    return str(value).replace("$", "$$").replace('"', '$\\"')


def verify_windows_installer(installer, stage, version, work):
    import winreg

    token = uuid.uuid4().hex
    identity = f"PaperCraft-verify-{token}"
    install = work / identity
    key = rf"Software\Microsoft\Windows\CurrentVersion\Uninstall\{identity}"
    programs = ctypes.create_unicode_buffer(32768)
    assert ctypes.windll.shell32.SHGetFolderPathW(None, 2, None, 0, programs) == 0
    shortcut = Path(programs.value) / f"{identity}.lnk"
    access = winreg.KEY_READ | winreg.KEY_WOW64_64KEY

    def registration_exists():
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, access):
                return True
        except FileNotFoundError:
            return False

    assert not install.exists() and not shortcut.exists() and not registration_exists()
    uninstaller = install / "Uninstall.exe"

    def uninstall():
        if uninstaller.exists():
            # NSIS _?= prevents its usual asynchronous self-copy. Keep this last,
            # unquoted (also for paths containing spaces) per the NSIS contract.
            command = subprocess.list2cmdline([str(uninstaller), f"/VERIFY={token}", "/S"])
            run_native(command + f" _?={install}")

    try:
        # NSIS /D= likewise must be the last, unquoted command-line parameter.
        command = subprocess.list2cmdline([str(installer), f"/VERIFY={token}", "/S"])
        run_native(command + f" /D={install}")
        verify_tree(stage, install, extra_files=("Uninstall.exe",))
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, access) as registration:
            values = {name: winreg.QueryValueEx(registration, name)[0] for name in
                      ("DisplayName", "DisplayVersion", "InstallLocation", "UninstallString",
                       "QuietUninstallString", "NoModify", "NoRepair")}
        assert values["DisplayName"] == "PaperCraft" and values["DisplayVersion"] == version
        assert Path(values["InstallLocation"]) == install
        expected_command = f'"{uninstaller}" /VERIFY={token}'
        assert values["UninstallString"] == expected_command
        assert values["QuietUninstallString"] == expected_command + " /S"
        assert values["NoModify"] == values["NoRepair"] == 1
        assert shortcut.is_file(), "Missing per-user Start-menu shortcut"
        escaped_link = str(shortcut).replace("'", "''")
        script = f"$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{escaped_link}'); $s.TargetPath"
        target = run_native(["powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive",
                             "-Command", script], timeout=30, stdout=subprocess.PIPE, text=True).stdout.strip()
        assert Path(target) == install / "papercraft.exe", "Shortcut target differs"
    finally:
        try:
            uninstall()
        finally:
            # Even on failure, clean only this random smoke identity, and still
            # fail verification if cleanup had to repair an incomplete uninstall.
            leftovers = []
            if registration_exists():
                winreg.DeleteKeyEx(winreg.HKEY_CURRENT_USER, key, winreg.KEY_WOW64_64KEY)
                leftovers.append("registration")
            if shortcut.exists():
                shortcut.unlink()
                leftovers.append("shortcut")
            if leftovers:
                raise RuntimeError("Windows uninstall left verification " + ", ".join(leftovers))
    assert not registration_exists() and not shortcut.exists()
    assert {p.relative_to(install) for p in install.rglob("*")} <= {Path("Uninstall.exe")}, "Uninstall left payload files"
    uninstaller.unlink(missing_ok=True)
    if install.exists():
        install.rmdir()
    print(f"Verified {installer.name}: silent per-user install, bytes, notices, HKCU registration, shortcut and uninstall.")


def package_windows(stage, destination, stem, version, work):
    installer = destination / f"{stem}-setup.exe"
    files = sorted((p.relative_to(stage) for p in stage.rglob("*") if p.is_file()), key=str)
    directories = sorted((p.relative_to(stage) for p in stage.rglob("*") if p.is_dir()),
                         key=lambda p: (len(p.parts), str(p)), reverse=True)
    replacements = {
        "@OUTPUT@": nsis_literal(installer), "@STAGE@": nsis_literal(stage), "@VERSION@": version,
        "@REMOVE_FILES@": "\n".join(f'  Delete "$INSTDIR\\{nsis_literal(p)}"' for p in files),
        "@REMOVE_DIRECTORIES@": "\n".join(f'  RMDir "$INSTDIR\\{nsis_literal(p)}"' for p in directories),
    }
    script = (ROOT / "packaging/windows/installer.nsi").read_text(encoding="utf-8")
    for marker, value in replacements.items():
        script = script.replace(marker, value)
    source = work / "installer.nsi"
    source.write_text(script, encoding="utf-8-sig")
    run_native(["makensis", "/V3", str(source)])
    verify_windows_installer(installer, stage, version, work)
    return installer


def package_installer(stage, destination, stem, version, platform):
    with tempfile.TemporaryDirectory(prefix="papercraft-installer-") as temporary:
        work = Path(temporary)
        if platform.startswith("macos-"):
            return package_dmg(stage, destination, stem, version, work)
        if platform.startswith("linux-"):
            return package_deb(stage, destination, stem, version, platform, work)
        return package_windows(stage, destination, stem, version, work)


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
        run_native(["codesign", "--force", "--deep", "--sign", "-", str(app)])
        run_native(["codesign", "--verify", "--deep", "--strict", str(app)])
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
    installer = package_installer(stage, destination, stem, version, args.platform)
    checksums.append(f"{digest(installer)}  {installer.name}\n")
    (destination / f"SHA256SUMS-{args.platform}.txt").write_text("".join(checksums), encoding="utf-8")


if __name__ == "__main__":
    main()
