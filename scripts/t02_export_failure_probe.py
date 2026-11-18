"""Verify parent and terminal-style group SIGINT never publish incomplete WAVs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("report")
report = Path(parser.parse_args().report)
assert not report.exists()
binary = Path("target/debug/clef").resolve()
records = []
for grouped in [False, True]:
    with tempfile.TemporaryDirectory(prefix="clef-cancel-", dir="/private/tmp") as directory:
        target = Path(directory)/"cancelled.wav"
        child = subprocess.Popen([str(binary), "render", "@fixture", str(target)],
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 text=True, start_new_session=True)
        record = dict(signal_scope="process_group" if grouped else "parent")
        try:
            deadline = time.monotonic()+15
            while not list(Path(directory).glob("*.clef-*-partial")) and time.monotonic() < deadline:
                assert child.poll() is None, "render ended before cancellation could be tested"
                time.sleep(0.001)
            assert list(Path(directory).glob("*.clef-*-partial")), "render never started"
            if grouped:
                os.killpg(child.pid, signal.SIGINT)
            else:
                os.kill(child.pid, signal.SIGINT)
            stdout, stderr = child.communicate(timeout=15)
            record.update(returncode=child.returncode, stdout=stdout, stderr=stderr,
                          final_exists=target.exists(),
                          partial_exists=bool(list(Path(directory).glob("*.clef-*-partial"))))
            assert child.returncode != 0
            assert "render cancelled" in stderr, stderr
            assert not record["final_exists"] and not record["partial_exists"]
            record["passed"] = True
        except Exception as error:
            record.update(passed=False, error=str(error))
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            records.append(record)
result = dict(binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), cases=records,
              passed=all(case["passed"] for case in records),
              scope="Actual SIGINT while rendering isolated fixtures; parent-only and terminal process-group paths.")
with report.open("x") as output:
    json.dump(result, output, indent=2)
print(json.dumps(result, indent=2))
if not result["passed"]:
    raise SystemExit(1)
