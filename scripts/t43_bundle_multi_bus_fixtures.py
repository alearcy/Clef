"""Bundle the four locally built T43 multi-bus instrument fixtures without installing plugins.

Build them first (each feature in its own target directory):
  cargo build -p vst3-host-testplug --features clef-multi-bus-fixture --target-dir target/t43-clef-multi-bus-fixture --locked --offline
  cargo build -p vst3-host-testplug --features clef-mono-main-fixture --target-dir target/t43-clef-mono-main-fixture --locked --offline
  cargo build -p clef-clap-latency-fixture --features multi-bus-fixture --target-dir target/t43-clap-multi-bus-fixture --locked --offline
  cargo build -p clef-clap-latency-fixture --features mono-main-fixture --target-dir target/t43-clap-mono-main-fixture --locked --offline
"""
from pathlib import Path
import plistlib
import shutil

for name, extension, build, library in [
    ("ClefMultiBusVST3", "vst3", "t43-clef-multi-bus-fixture", "libvst3_host_testplug.dylib"),
    ("ClefMonoMainVST3", "vst3", "t43-clef-mono-main-fixture", "libvst3_host_testplug.dylib"),
    ("ClefMultiBusCLAP", "clap", "t43-clap-multi-bus-fixture", "libclef_clap_latency_fixture.dylib"),
    ("ClefMonoMainCLAP", "clap", "t43-clap-mono-main-fixture", "libclef_clap_latency_fixture.dylib"),
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
