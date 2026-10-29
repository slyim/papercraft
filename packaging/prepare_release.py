"""Verify all five CI artifact payloads before preparing a new preview release."""
import hashlib
from pathlib import Path
import shutil
import sys

source, destination, version = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
platforms = {"linux-x86_64": 2, "linux-arm64": 2, "macos-arm64": 3,
             "macos-x86_64": 3, "windows-x86_64": 2}
verified = {}
for platform, count in platforms.items():
    folder = source / platform
    manifest = folder / f"SHA256SUMS-{platform}.txt"
    lines = manifest.read_text(encoding="utf-8").splitlines()
    assert len(lines) == count, f"Incomplete manifest: {platform}"
    for line in lines:
        digest, name = line.split("  ", 1)
        stem = f"papercraft-{version}-{platform}"
        assert name == Path(name).name and (name.startswith(stem + ".") or
                platform == "windows-x86_64" and name == stem + "-setup.exe")
        assert name not in verified, f"Duplicate payload: {name}"
        path = folder / name
        assert path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == digest, f"Checksum mismatch: {name}"
        verified[name] = (digest, path)
assert len(verified) == 12
destination.mkdir(parents=True, exist_ok=False)
for name, (_, path) in verified.items():
    shutil.copyfile(path, destination / name)
(destination / "SHA256SUMS.txt").write_text(
    "".join(f"{digest}  {name}\n" for name, (digest, _) in sorted(verified.items())),
    encoding="utf-8", newline="\n")
shutil.copyfile(Path(__file__).resolve().parent.parent / "LICENSE-MIT", destination / "LICENSE-MIT")
print("Verified 12 payloads; prepared 14 release assets.")
