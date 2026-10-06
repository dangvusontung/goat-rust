#!/usr/bin/env python3
"""Measure one native child process; usage: measure-native.py OUTPUT COMMAND [ARGS]."""
import os
import subprocess
import sys
import time

if len(sys.argv) < 3:
    raise SystemExit("Usage: measure-native.py OUTPUT COMMAND [ARGS]")
with open(sys.argv[1], "w", encoding="utf-8") as output:
    begin = time.monotonic()
    child = subprocess.Popen(sys.argv[2:], stdout=output, stderr=subprocess.STDOUT)
    _, wait_status, usage = os.wait4(child.pid, 0)
    child.returncode = os.waitstatus_to_exitcode(wait_status)
    peak_kib = usage.ru_maxrss / 1024 if sys.platform == "darwin" else usage.ru_maxrss
    print(
        f"process_wall_seconds={time.monotonic() - begin:.6f} "
        f"user_seconds={usage.ru_utime:.6f} system_seconds={usage.ru_stime:.6f} "
        f"peak_rss_kib={peak_kib} exit={child.returncode}",
        file=output,
    )
sys.exit(child.returncode)
