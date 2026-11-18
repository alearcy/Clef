"""Exercise Clef's actual live supervisor with internal exit/hang fixtures only."""
import argparse
import json
import hashlib
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("report")
parser.add_argument("--device", type=int, required=True)
args = parser.parse_args()
report = Path(args.report)
assert not report.exists()
records = []
with tempfile.TemporaryDirectory(prefix="clef-recovery-", dir="/private/tmp") as directory:
    with report.with_suffix(".log").open("x") as log:
        child = subprocess.Popen([str(Path("target/debug/clef").resolve()), "plugin", "@fixture"],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                 text=True, bufsize=1, start_new_session=True)
        lines, reports = [], []
        lock = threading.Lock()
        def read():
            for line in child.stdout:
                log.write(line)
                log.flush()
                with lock:
                    lines.append((time.monotonic(), line))
                    if "{" in line:
                        try:
                            value = json.loads(line[line.index("{"):])
                            if "played_frames" in value:
                                reports.append((time.monotonic(), value))
                        except ValueError:
                            pass
        reader = threading.Thread(target=read, daemon=True)
        reader.start()
        def send(command):
            child.stdin.write(command + "\n")
            child.stdin.flush()
        def wait_line(text, after, timeout):
            until = time.monotonic()+timeout
            while time.monotonic() < until:
                with lock:
                    found = [entry for entry in lines if entry[0] >= after and text in entry[1]]
                if found:
                    return found[-1][0]
                time.sleep(0.01)
            raise TimeoutError(text)
        def status():
            started = time.monotonic()
            send("status")
            until = started+2
            while time.monotonic() < until:
                with lock:
                    found = [entry for entry in reports if entry[0] >= started]
                if found:
                    return found[-1][1]
                time.sleep(0.01)
            raise TimeoutError("supervisor status became unresponsive")
        result = {"binary_sha256": hashlib.sha256(Path("target/debug/clef").read_bytes()).hexdigest(), "scope": "Actual Clef supervisor + CPAL + controlled internal worker; no commercial plugin crash induced."}
        try:
            state = Path(directory)/"saved.state"
            send("fixture-edit 1")
            send(f"save {state}")
            for fault in ["exit", "hang"]:
                started = time.monotonic()
                send(f"start {args.device} 1 2")
                wait_line("Audio output:", started, 10)
                time.sleep(1)
                before = status()
                send("fixture-edit 2")
                injected = time.monotonic()
                send("fault-"+fault)
                observed = wait_line("Live worker failed:", injected, 2)
                after = status()
                assert after["worker_failed"]
                time.sleep(0.1)
                continuing = status()
                assert continuing["played_frames"] > after["played_frames"], "device callback stopped"
                restart = time.monotonic()
                send("recreate")
                wait_line("Recreated worker from last saved state", restart, 10)
                send(f"start {args.device} 1 2")
                wait_line("Audio output:", restart, 10)
                time.sleep(1)
                recovered = status()
                assert not recovered["worker_failed"]
                assert recovered["deadline_missed_blocks"] == 0
                assert recovered["output_errors"] == 0
                send("stop")
                restored = Path(directory)/f"restored-{fault}.state"
                send(f"save {restored}")
                until = time.monotonic()+2
                while not restored.exists() and time.monotonic() < until:
                    time.sleep(0.01)
                assert restored.read_bytes() == bytes([1]), "unsaved edit survived recovery or saved state was lost"
                time.sleep(0.1)
                records.append({"fault": fault, "saved_state_restored": True, "unsaved_edits_discarded": True, "failure_report_seconds": observed-injected,
                                "before": before, "failed": after, "continuing": continuing,
                                "recovered": recovered})
            assert state.read_bytes() == bytes([1])
            send("quit")
            child.wait(timeout=10)
            assert child.returncode == 0
            result.update(cases=records, passed=True)
        except Exception as error:
            result.update(cases=records, error=str(error), passed=False)
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            reader.join(5)
            with report.open("x") as out:
                json.dump(result, out, indent=2)
            print(json.dumps(result, indent=2))
        if not result["passed"]:
            raise SystemExit(1)
