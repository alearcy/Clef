"""Real format notification, lifecycle and impulse-alignment proof, without a device."""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from t02_worker_probe import exchange, receive

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("plugin")
parser.add_argument("report")
args = parser.parse_args()
report = Path(args.report)
assert not report.exists()
binary = str(Path("target/debug/clef").resolve())
plugin = str(Path(args.plugin).resolve())
with tempfile.TemporaryDirectory(prefix="clef-latency-", dir="/private/tmp") as directory:
    address = str(Path(directory)/"rpc")
    with socket.socket(socket.AF_UNIX) as listener, report.with_suffix(".log").open("x") as log:
        listener.bind(address)
        listener.listen(2)
        listener.settimeout(65)
        child = subprocess.Popen([binary, "worker", address, plugin], stdout=log, stderr=log)
        try:
            control, _ = listener.accept()
            audio, _ = listener.accept()
            hello = receive(control, time.monotonic()+65)
            def start(offline):
                exchange(audio, "SuspendAudio", 1)
                return exchange(control, {"Start": {"offline": offline}}, 1)
            def process(position, events):
                return exchange(audio, {"Process": {"position": position, "events": events, "frames": 128}}, 1)
            def impulse(latency):
                samples = []
                for index in range(4):
                    reply = process(index*128, [{"offset": 37, "bytes": [144,60,100]}] if index == 0 else [])
                    samples.extend(reply["frames"])
                assert samples[37+latency] == [0.25, 0.25]
                assert all(frame == [0.0,0.0] for i,frame in enumerate(samples) if i != 37+latency)
                return samples
            assert start(False)["latency"] == 0
            before = impulse(0)
            notified = False
            for i in range(1000):
                reply = process(512+i*128, [])
                if reply["restart"]:
                    notified = True
                    break
                time.sleep(0.001)
            assert notified, "no actual format restart notification"
            transition = time.monotonic()
            exchange(audio, "SuspendAudio", 1)
            exchange(control, "Stop", 1)
            assert exchange(control, {"Start": {"offline": False}}, 1)["latency"] == 257
            gap = time.monotonic()-transition
            after = impulse(257)
            exchange(audio, "SuspendAudio", 1)
            exchange(control, "Stop", 1)
            assert start(True)["latency"] == 257
            offline = impulse(257)
            assert max(abs(a-b) for x,y in zip(after,offline) for a,b in zip(x,y)) <= 1e-6
            assert before[37:] == after[294:] + [[0.0,0.0]]*257
            exchange(audio, "SuspendAudio", 1)
            exchange(control, "Stop", 1)
            exchange(control, "Quit", 1)
            child.wait(timeout=5)
            assert child.returncode == 0
            result = dict(plugin=plugin, name=hello["name"], passed=True, old_latency=0,
                          new_latency=257, impulse_offset=37, transition_control_seconds=gap,
                          live_core_offline_max_error=0, scope="Worker processing and actual format notification; device gap is measured separately.")
            report.write_text(json.dumps(result, indent=2))
            print(json.dumps(result, indent=2))
        finally:
            if child.poll() is None:
                child.kill()
            child.wait()
