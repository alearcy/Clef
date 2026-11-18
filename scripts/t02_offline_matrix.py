"""Render the final selected states and inspect complete PCM24 WAVs; no listening claim."""
import hashlib
import json
from pathlib import Path
import subprocess
import wave

report = Path("reports/T02-offline-final.json")
assert not report.exists()
binary = Path("target/debug/clef").resolve()
cases = [
    ("pianoverse", "/Library/Audio/Plug-Ins/VST3/Pianoverse.vst3", "pianoverse-AA-Y7-width63.state"),
    ("zebrahz-vst3", "/Library/Audio/Plug-Ins/VST3/ZebraHZ.vst3", "zebrahz-vst3-initialize-master20.state"),
    ("zebrahz-clap", "/Library/Audio/Plug-Ins/CLAP/u-he/ZebraHZ.clap", "zebrahz-clap-initialize-master20.state"),
]
output = Path("validation-output/exports")
output.mkdir(parents=True, exist_ok=True)
records = []
try:
    for name, plugin, snapshot in cases:
        state = Path("validation-output/states") / snapshot
        wav = (output / f"T02-{name}-final.wav").resolve()
        assert not wav.exists(), "choose a fresh export path"
        with (output / f"T02-{name}-final.log").open("x") as log:
            result = subprocess.run(["rtk", "proxy", str(binary), "render", plugin, str(wav), str(state)],
                                    stdout=log, stderr=subprocess.STDOUT, timeout=180)
        assert result.returncode == 0, f"render failed: {name}"
        peak, nonzero, last_second_peak = 0, 0, 0
        with wave.open(str(wav)) as reader:
            assert (reader.getnchannels(), reader.getframerate(), reader.getsampwidth(), reader.getnframes()) == (2, 48000, 3, 2323200)
            position = 0
            while data := reader.readframes(4096):
                for i in range(0, len(data), 3):
                    sample = abs(int.from_bytes(data[i:i+3], "little", signed=True))
                    peak = max(peak, sample)
                    nonzero += sample != 0
                    if position + i//6 >= 2323200 - 48000:
                        last_second_peak = max(last_second_peak, sample)
                position += len(data)//6
            assert position == 2323200 and nonzero > 0
            assert peak < 8388607, "clipped PCM24 output"
        records.append(dict(case=name, path=str(wav), frames=2323200, channels=2, rate=48000,
                            sample_bytes=3, peak_pcm24=peak, nonzero_samples=nonzero,
                            last_second_peak_pcm24=last_second_peak,
                            sha256=hashlib.sha256(wav.read_bytes()).hexdigest(),
                            state_sha256=hashlib.sha256(state.read_bytes()).hexdigest()))
        print(name, "export verified", flush=True)
finally:
    with report.open("x") as target:
        json.dump(dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), cases=records,
                       all_three_passed=len(records)==3,
                       scope="Final export format/readability/sample checks; listening and tail acceptance require user confirmation."), target, indent=2)
