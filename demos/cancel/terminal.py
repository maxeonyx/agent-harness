#!/usr/bin/env python3
"""Run a command in a pseudo-terminal and press Ctrl-C at it, as Max would.

The terminal's own line discipline turns ^C into SIGINT for its foreground
process group, so whatever shares the command's group gets it too: the run
shows what reaches whom.

Usage: terminal.py LOG PATTERN DELAY PRESSES GAP -- COMMAND...
Once a line of output contains PATTERN, wait DELAY seconds, then press
Ctrl-C PRESSES times, GAP seconds apart. Prints the output with seconds
since the start, and how long after the first press the command exited.
"""
import os
import pty
import select
import sys
import time

log, pattern, delay, presses, gap = sys.argv[1:6]
command = sys.argv[sys.argv.index("--") + 1 :]
delay, presses, gap = float(delay), int(presses), float(gap)

start = time.monotonic()
pid, fd = pty.fork()
if pid == 0:
    os.execvp(command[0], command)

out = open(log, "w")
pending = b""
due = None
pressed = 0
first = None


def say(line):
    stamped = f"{time.monotonic() - start:7.2f}  {line}"
    print(stamped, flush=True)
    out.write(stamped + "\n")
    out.flush()


while True:
    now = time.monotonic()
    timeout = None if due is None else max(0.0, due - now)
    ready, _, _ = select.select([fd], [], [], timeout)
    if due is not None and time.monotonic() >= due and pressed < presses:
        os.write(fd, b"\x03")
        pressed += 1
        if first is None:
            first = time.monotonic()
        say(f"[the terminal] Ctrl-C ({pressed})")
        due = time.monotonic() + gap if pressed < presses else None
    if not ready:
        continue
    try:
        data = os.read(fd, 4096)
    except OSError:
        data = b""
    if not data:
        break
    pending += data
    while b"\n" in pending:
        line, pending = pending.split(b"\n", 1)
        text = line.decode(errors="replace").rstrip("\r")
        say(text)
        if due is None and pressed == 0 and pattern in text:
            due = time.monotonic() + delay

_, status = os.waitpid(pid, 0)
code = os.waitstatus_to_exitcode(status)
after = "" if first is None else f", {time.monotonic() - first:.2f}s after the first Ctrl-C"
say(f"[the terminal] the command exited with {code}{after}")
