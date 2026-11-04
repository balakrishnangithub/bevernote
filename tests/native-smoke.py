"""Bounded Windows startup check; does not assert rendered GUI interaction."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import time


ROOT = Path(__file__).resolve().parent.parent
EXECUTABLE = ROOT / "target" / "release" / "bevernote.exe"
DATA = ROOT / "test-results" / "native-smoke"
DATA.mkdir(parents=True, exist_ok=True)

environment = os.environ.copy()
environment["APPDATA"] = str(DATA / "roaming")
environment["LOCALAPPDATA"] = str(DATA / "local")

process = subprocess.Popen(
    [str(EXECUTABLE)],
    cwd=ROOT,
    env=environment,
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
try:
    time.sleep(8)
    return_code = process.poll()
    if return_code is not None:
        raise RuntimeError(f"Bevernote exited during startup with code {return_code}")
    print("PASS: Windows executable remained running for 8 seconds with an isolated data directory.")
    print("GUI rendering and native interaction were not verified by this check.")
finally:
    if process.poll() is None:
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        process.wait(timeout=15)
