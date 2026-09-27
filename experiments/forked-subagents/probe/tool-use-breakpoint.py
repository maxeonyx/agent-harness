"""Can the shared part live only in the parent's `task` call, with the breakpoint on that tool_use block?"""
import json, sqlite3, sys, time, urllib.request, os
cred = json.loads(sqlite3.connect(f"file:{os.path.expanduser('~/.local/share/opencode/opencode.db')}?mode=ro", uri=True)
    .execute("select value from credential where integration_id='anthropic' order by time_updated desc limit 1").fetchone()[0])
assert cred["expires"] / 1000 > time.time() + 120, "token expired"
def call(body):
    req = urllib.request.Request("https://api.anthropic.com/v1/messages", data=json.dumps(body).encode(), headers={
        "authorization": f"Bearer {cred['access']}", "anthropic-version": "2023-06-01", "anthropic-beta": "oauth-2025-04-20", "content-type": "application/json"})
    try:
        with urllib.request.urlopen(req) as r:
            u = json.load(r)["usage"]
            return {k: u[k] for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")}
    except urllib.error.HTTPError as e:
        return f"{e.code} {e.read().decode()[:300]}"
run = sys.argv[1]
breakpoint = sys.argv[2] == "breakpoint"
parent_facts = "\n".join(f"Parent note {i}: the {run} value of slot p{i} is {i*37 % 991}." for i in range(260))
shared = "\n".join(f"Shared rule {i} ({run}): items tagged r{i} weigh {i*53 % 877} grams." for i in range(220))
system = [{"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}, {"type": "text", "text": "You are an agent."}]
tools = [{"name": "task", "description": "split work", "input_schema": {"type": "object", "properties": {"shared": {"type": "string"}, "agents": {"type": "array", "items": {"type": "object"}}}}}]
def child(name):
    task_call = {"type": "tool_use", "id": "toolu_probe1", "name": "task",
                 "input": {"shared": shared, "agents": [{"name": "a", "task": "weight of r3"}, {"name": "b", "task": "weight of r5"}]}}
    if breakpoint:
        task_call["cache_control"] = {"type": "ephemeral"}
    own = f"You are {name}. Report only the weight of r{3 if name == 'a' else 5}."
    return {"model": "claude-sonnet-5", "max_tokens": 200, "system": system, "tools": tools, "cache_control": {"type": "ephemeral"},
            "messages": [{"role": "user", "content": [{"type": "text", "text": parent_facts + "\nSplit this into two agents."}]},
                         {"role": "assistant", "content": [task_call]},
                         {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_probe1", "content": own}]}]}
for name in ["a", "b", "b"]:
    print(name, call(child(name)))
    time.sleep(2)
