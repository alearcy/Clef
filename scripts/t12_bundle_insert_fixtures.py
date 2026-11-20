"""Bundle the two locally built T12 effects without installing plugins."""
from pathlib import Path
import plistlib
import shutil

for name, extension, build, library in [
    ("ClefInsertVST3", "vst3", "t12-vst3-fixture", "libvst3_host_testplug.dylib"),
    ("ClefInsertCLAP", "clap", "t12-clap-fixture", "libclef_clap_latency_fixture.dylib"),
]:
    source = Path("target") / build / "debug" / library
    bundle = Path("validation-output/fixtures") / f"{name}.{extension}"
    contents = bundle / "Contents"
    binary = contents / "MacOS" / name
    binary.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, binary)
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleExecutable": name,
            "CFBundleIdentifier": f"dev.clef.{name}",
            "CFBundleName": name,
            "CFBundlePackageType": "BNDL",
            "CFBundleVersion": "1",
        }, output)
    assert binary.read_bytes() == source.read_bytes()
    print(bundle)
