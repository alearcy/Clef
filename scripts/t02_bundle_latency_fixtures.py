"""Bundle explicitly built T02 latency fixtures on macOS; no installed plugins changed."""
from pathlib import Path
import plistlib
import shutil
import sys

assert sys.platform == "darwin"
for name, extension, source in [
    ("ClefLatencyVST3", "vst3", "target/latency-vst3/debug/libvst3_host_testplug.dylib"),
    ("ClefLatencyCLAP", "clap", "target/debug/libclef_clap_latency_fixture.dylib"),
]:
    source = Path(source)
    assert source.is_file(), f"build the fixture first: {source}"
    bundle = Path("validation-output/fixtures") / f"{name}.{extension}"
    executable = bundle / "Contents/MacOS" / name
    executable.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, executable)
    with (bundle / "Contents/Info.plist").open("wb") as output:
        plistlib.dump(dict(CFBundleExecutable=name, CFBundleIdentifier=f"dev.clef.{name}",
                          CFBundleName=name, CFBundlePackageType="BNDL", CFBundleVersion="1"), output)
    assert executable.read_bytes() == source.read_bytes()
    print(bundle)
