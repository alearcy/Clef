"""Timed T02 live validation; records software counters, never claims listening."""
import argparse
import json
import hashlib
import os
import signal
from pathlib import Path
import subprocess
import threading
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("plugin")
parser.add_argument("report")
parser.add_argument("--state")
parser.add_argument("--editor-cycles", type=int, default=0)
parser.add_argument("--binary", default="target/debug/clef")
parser.add_argument("--device", type=int, required=True)
parser.add_argument("--pair", type=int, nargs=2, default=[1, 2])
parser.add_argument("--midi", type=int)
parser.add_argument("--seconds", type=int, required=True)
args = parser.parse_args()
assert args.seconds > 0 and all(ch > 0 for ch in args.pair)
report = Path(args.report)
assert not report.exists(), "report already exists"
log_path = report.with_suffix(".log")
updates = []
editor_acks, errors = [], []
lock = threading.Lock()
ready = threading.Event()
command = [str(Path(args.binary).resolve()), "plugin", str(Path(args.plugin).resolve())]
if args.state:
    command.append(str(Path(args.state).resolve()))
with log_path.open("x") as log:
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, text=True, bufsize=1, start_new_session=True)
    def read():
        for line in child.stdout:
            log.write(line)
            log.flush()
            if "Error:" in line:
                errors.append(line.strip())
            if "Editor opened." in line or "Editor closed." in line:
                editor_acks.append(line.strip())
            if "Audio output:" in line:
                print(line.strip(), flush=True)
                ready.set()
            if "{" in line:
                try:
                    value = json.loads(line[line.index("{"):])
                    if "played_frames" in value:
                        with lock:
                            updates.append((time.monotonic(), value))
                except ValueError:
                    pass
    reader = threading.Thread(target=read, daemon=True)
    reader.start()
    def send(command):
        child.stdin.write(command + "\n")
        child.stdin.flush()
    result = {"binary_sha256": hashlib.sha256(Path(command[0]).read_bytes()).hexdigest(),
              "state_sha256": hashlib.sha256(Path(args.state).read_bytes()).hexdigest() if args.state else None,
              "plugin": args.plugin, "state": args.state, "counter_checks_passed": False,
              "midi_port_index": args.midi, "requested_seconds": args.seconds,
              "scope": "Live software counters; user listening and physical gestures recorded separately."}
    try:
        mode = "midi" if args.midi is not None else "start"
        start = f"{mode} {args.device} {args.pair[0]} {args.pair[1]}"
        if args.midi is not None:
            start += f" {args.midi}"
        send(start)
        assert ready.wait(65), "audio stream did not open"
        print("Ready: live stream open; warm-up five seconds.", flush=True)
        for _ in range(5):
            time.sleep(1)
        send("status")
        deadline = time.monotonic() + 5
        while not updates and time.monotonic() < deadline:
            time.sleep(0.01)
        assert updates, "missing initial callback report"
        with lock:
            baseline = updates[-1][1]
        print("Measurement started: play now." if args.midi is not None else "Measurement started.", flush=True)
        target = baseline["played_frames"] + args.seconds * 48000
        until = time.monotonic() + args.seconds + 20
        editor_step = 0
        while time.monotonic() < until:
            if editor_step < args.editor_cycles * 2:
                send("open" if editor_step % 2 == 0 else "close")
                editor_step += 1
            time.sleep(1)
            send("status")
            with lock:
                latest = updates[-1][1]
            assert not latest["worker_failed"], "worker failed during timed test"
            if latest["played_frames"] >= target:
                break
        assert latest["played_frames"] >= target, "insufficient callback frames"
        send("stop")
        send("quit")
        child.wait(timeout=15)
        reader.join(5)
        with lock:
            final = updates[-1][1]
        result.update(baseline=baseline, final=final,
                      measured_frames=final["played_frames"] - baseline["played_frames"])
        result["deltas"] = {k: final[k]-baseline[k] for k in
                            ["deadline_missed_blocks", "output_errors", "discarded_stale_blocks",
                             "midi_received", "midi_overflows", "late_midi_events"]}
        result["editor_cycles_acknowledged"] = len(editor_acks) // 2
        assert len(editor_acks) == args.editor_cycles * 2, "incomplete editor lifecycle acknowledgements"
        assert not errors, errors
        assert result["deltas"]["deadline_missed_blocks"] == 0, "callback deadline misses"
        assert result["deltas"]["output_errors"] == 0, "audio output errors"
        assert result["deltas"]["midi_overflows"] == 0, "MIDI overflow"
        if args.midi is not None:
            assert result["deltas"]["midi_received"] > 0, "no physical MIDI input observed"
        assert child.returncode == 0, "console failed"
        result["counter_checks_passed"] = True
    except (Exception, KeyboardInterrupt) as error:
        result["error"] = str(error) or "Measurement interrupted"
        print("Timed test failed:", error, flush=True)
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
        reader.join(5)
        with lock:
            if updates:
                result.setdefault("last_observation", updates[-1][1])
        with report.open("x") as output:
            json.dump(result, output, indent=2)
        print(json.dumps(result, indent=2), flush=True)
    if "error" in result:
        raise SystemExit(1)
