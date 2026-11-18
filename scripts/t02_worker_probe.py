"""Diagnostic only: editor lifecycle with concurrent worker processing, no audio device.

Run after building Clef, outside a sandbox that blocks Unix sockets/WindowServer.
No listening, callback deadline, or hardware acceptance is implied.
"""
import argparse
import json
import math
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import threading
import time

MAX_PACKET = 64 * 1024 * 1024 * 4 + 8192


def exchange(stream, request, timeout=60):
    deadline = time.monotonic() + timeout
    data = json.dumps(request).encode()
    assert len(data) <= MAX_PACKET
    stream.settimeout(timeout)
    stream.sendall(struct.pack("<I", len(data)) + data)
    return receive(stream, deadline)


def receive(stream, deadline):
    def read(count):
        data = bytearray()
        while len(data) < count:
            stream.settimeout(max(0.001, deadline - time.monotonic()))
            if time.monotonic() >= deadline:
                raise TimeoutError("whole worker exchange exceeded deadline")
            chunk = stream.recv(count - len(data))
            if not chunk:
                raise EOFError("worker disconnected")
            data.extend(chunk)
        return data
    size = struct.unpack("<I", read(4))[0]
    assert 0 < size <= MAX_PACKET, "invalid worker packet size"
    reply = json.loads(read(size))
    if reply["error"]:
        raise RuntimeError(reply["error"])
    return reply


def probe(binary, plugin, state, report, sequence=False):
    timeline = json.loads(subprocess.check_output([binary, "sequence"])) if sequence else None
    timings, editor_ms, failures = [], [], []
    peak = 0.0
    stop = threading.Event()
    with tempfile.TemporaryDirectory(prefix="clef-probe-", dir="/private/tmp") as directory:
        address = str(Path(directory) / "rpc")
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(address)
            listener.listen(2)
            listener.settimeout(65)
            with tempfile.TemporaryFile() as log:
                process = subprocess.Popen([binary, "worker", address, plugin], stdout=log, stderr=log)
                control = audio = thread = None
                completed = 0
                try:
                    control, _ = listener.accept()
                    audio, _ = listener.accept()
                    hello = receive(control, time.monotonic() + 65)
                    if state:
                        data = Path(state).read_bytes()
                        assert len(data) <= 64 * 1024 * 1024
                        exchange(audio, "SuspendAudio", 2)
                        exchange(control, {"Restore": {"state": list(data)}})
                    exchange(audio, "SuspendAudio", 2)
                    exchange(control, {"Start": {"offline": False}})

                    def processing():
                        nonlocal peak
                        position = 230400
                        pending = iter(timeline["events"]) if timeline else iter(())
                        event = next(pending, None)
                        elapsed = 0
                        try:
                            while not stop.is_set() and (not timeline or elapsed < timeline["length"]):
                                start = time.monotonic()
                                events = [{"offset": 37, "bytes": [144, 60, 80]}] if position == 230400 else []
                                if timeline:
                                    events = []
                                    while event and event["offset"] < elapsed + 128:
                                        events.append({"offset": event["offset"] - elapsed, "bytes": event["bytes"]})
                                        event = next(pending, None)
                                reply = exchange(audio, {"Process": {"position": position, "events": events, "frames": 128}}, 2)
                                assert len(reply["frames"]) == 128
                                assert all(len(frame) == 2 and all(math.isfinite(v) for v in frame) for frame in reply["frames"])
                                peak = max(peak, max(abs(v) for frame in reply["frames"] for v in frame))
                                timings.append((time.monotonic() - start) * 1000)
                                position += 128
                                elapsed += 128
                                stop.wait(max(0, 128 / 48000 - (time.monotonic() - start)))
                        except Exception as error:
                            failures.append(str(error))
                            stop.set()

                    thread = threading.Thread(target=processing)
                    thread.start()
                    time.sleep(0.3)
                    for cycle in range(10):
                        assert not failures, failures
                        for opened in [True, False]:
                            start = time.monotonic()
                            exchange(control, {"Editor": {"open": opened}})
                            editor_ms.append({"cycle": cycle + 1, "open": opened, "ms": (time.monotonic() - start) * 1000})
                            time.sleep(0.05)
                        completed += 1
                    if not timeline:
                        stop.set()
                    thread.join(60 if timeline else 3)
                    assert not thread.is_alive(), "processing thread did not stop"
                    assert not failures, failures
                    exchange(audio, "SuspendAudio", 2)
                    exchange(control, "Stop")
                    exchange(control, "Quit")
                    process.wait(timeout=5)
                    assert process.returncode == 0, f"worker exited {process.returncode}"
                    assert peak > 0, "worker generated only silence"
                    log.seek(0)
                    warnings = [line for line in log.read().decode(errors="replace").splitlines()
                                if "main-thread" in line.lower() or "main thread" in line.lower()
                                or "latency" in line.lower()]
                    result = {
                        "lifecycle_diagnostics": warnings,
                        "sequence": "full scheduled range" if timeline else "single held note",
                        "plugin": plugin, "name": hello["name"], "state": str(state) if state else None,
                        "latency_samples": hello["latency"], "cycles": 10, "processed_blocks": len(timings),
                        "peak": peak, "max_process_exchange_ms": max(timings),
                        "exchanges_over_128_frame_duration": sum(t > 128 / 48 for t in timings),
                        "editor_exchanges": editor_ms,
                        "scope": "Concurrent worker RPC diagnostic; no audio device, listening or hardware evidence.",
                    }
                    with Path(report).open("x") as output:
                        json.dump(result, output, indent=2)
                    print(json.dumps({k: v for k, v in result.items() if k != "editor_exchanges"}, indent=2))
                except Exception as error:
                    failure = {
                        "sequence": "full scheduled range" if timeline else "single held note",
                        "plugin": plugin, "error": str(error), "worker_returncode": process.poll(),
                        "completed_cycles": completed, "processing_failures": failures,
                        "processed_blocks": len(timings), "peak": peak,
                        "max_process_exchange_ms": max(timings, default=0),
                        "editor_exchanges": editor_ms,
                        "scope": "Failed worker-only diagnostic; no audio device or acceptance pass.",
                    }
                    with Path(report).open("x") as output:
                        json.dump(failure, output, indent=2)
                    print(json.dumps({k: v for k, v in failure.items() if k != "editor_exchanges"}, indent=2))
                    log.seek(0)
                    print(log.read().decode(errors="replace")[-5000:])
                    raise
                finally:
                    stop.set()
                    process.kill() if process.poll() is None else None
                    process.wait()
                    for stream in [control, audio]:
                        if stream:
                            stream.close()
                    if thread:
                        thread.join(3)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plugin")
    parser.add_argument("report")
    parser.add_argument("--state")
    parser.add_argument("--sequence", action="store_true", help="use Clef's complete scheduled sequence")
    parser.add_argument("--binary", default="target/debug/clef")
    args = parser.parse_args()
    probe(str(Path(args.binary).resolve()), str(Path(args.plugin).resolve()), args.state, args.report, args.sequence)
