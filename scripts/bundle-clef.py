#!/usr/bin/env python3
"""Bundle the explicitly built debug executable for local macOS GUI validation."""
from pathlib import Path
import os
import plistlib
import shutil

root = Path(__file__).resolve().parents[1]
source = root / "target/debug/clef"
if not source.is_file():
    raise SystemExit("Build first: rtk cargo build -p clef --locked")
contents = root / "target/Clef.app/Contents"
executable = contents / "MacOS/clef"
executable.parent.mkdir(parents=True, exist_ok=True)
temporary = executable.with_name(f"clef-{os.getpid()}.partial")
try:
    shutil.copy2(source, temporary)
    if source.read_bytes() != temporary.read_bytes():
        raise SystemExit("Executable verification failed")
    temporary.replace(executable)
finally:
    temporary.unlink(missing_ok=True)
(contents / "Info.plist").write_bytes(plistlib.dumps({
    "CFBundleExecutable": "clef",
    "CFBundleIdentifier": "com.arcidiaco.clef",
    "CFBundleName": "Clef",
    "CFBundleDisplayName": "Clef",
    "CFBundlePackageType": "APPL",
    "CFBundleShortVersionString": "0.1.0",
    "CFBundleVersion": "0.1.0",
    "NSHighResolutionCapable": True,
}))
print(root / "target/Clef.app")
