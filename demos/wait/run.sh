#!/usr/bin/env bash
# An orchestrator acts on each piece of its background work as it lands,
# and Max is answered while it waits. A scripted orchestrator starts three
# background spawns (forks that sleep 3, 14 and 22 s), then `wait`s
# (`until: any`, the default): each wait returns the next fork's whole
# result, and it reviews that one. Max writes 8 s in, during its second
# wait: his words end the wait at once, listing what still runs, and he is
# answered. The rest still wake it as they land. Free (scripted model),
# about 40 s; it leaves the brain up: `node dev/drive.mjs down --brain wait`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain wait "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
drive down --wipe >/dev/null
drive up
repo=$PWD/.drive.ignore/wait/repo
demos/brain/scratch.sh "$repo" >/dev/null
# The orchestrator's script: one line per response.
mandate=$(python3 - <<'PY'
import json
def fork(name, secs):
    bash = json.dumps({"command": f"sleep {secs}; echo {name} checked"})
    return json.dumps({"name": name, "tool": "spawn", "input": {"tasks": [
        {"name": name, "mandate": f"tool bash {bash}\nsay {name}: all green after {secs} s."}]}})
print("\n".join([
    " ".join(f"tool background {fork(n, s)}" for n, s in [("a", 3), ("b", 14), ("c", 22)]),
    "tool wait {}",
    'tool bash {"command": "echo reviewing a"}',
    "tool wait {}",
]))
PY
)
chat=$(drive chat orchestrator "$repo" "$mandate")
echo "== $(at) the orchestrator $chat starts a, b and c in the background, and waits"
sleep 8
echo "== $(at) a has landed and been reviewed; the orchestrator waits again:"
drive log "$chat"
echo "== $(at) Max writes during the wait"
drive say "$chat" 'say Heard you, Max: b and c still run, and I review each as it lands.
tool bash {"command": "echo reviewing b"}
tool wait {}
tool bash {"command": "echo reviewing c"}
say All three reviewed.'
drive settle "$chat" >/dev/null
echo "== $(at) answered at once; b and c still run:"
drive log "$chat"
echo "== $(at) waiting for b and c to land"
until drive log "$chat" | grep -q "^agent: All three reviewed"; do sleep 1; done
echo "== $(at) b woke it, it reviewed b, waited for c and reviewed it:"
drive log "$chat"
