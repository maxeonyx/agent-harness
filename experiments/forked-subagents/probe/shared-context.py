"""Does a text block inside a tool_result carry a cache breakpoint that siblings share?"""
import json, sqlite3, sys, time, urllib.request, os
cred = json.loads(sqlite3.connect(f"file:{os.path.expanduser('~/.local/share/opencode/opencode.db')}?mode=ro", uri=True)
    .execute("select value from credential where integration_id='anthropic' order by time_updated desc limit 1").fetchone()[0])
assert cred["expires"] / 1000 > time.time() + 120, "token expired"
TOK = cred["access"]
def call(body):
    req = urllib.request.Request("https://api.anthropic.com/v1/messages", data=json.dumps(body).encode(), headers={
        "authorization": f"Bearer {TOK}", "anthropic-version": "2023-06-01", "anthropic-beta": "oauth-2025-04-20", "content-type": "application/json"})
    try:
        with urllib.request.urlopen(req) as r:
            return json.load(r), r.headers.get("anthropic-ratelimit-unified-representative-claim")
    except urllib.error.HTTPError as e:
        return {"error": e.code, "body": e.read().decode()[:400]}, None
run = sys.argv[1]
parent_facts = "\n".join(f"Parent note {i}: the {run} value of slot p{i} is {i*37 % 991}." for i in range(260))
shared = "\n".join(f"Shared rule {i} ({run}): items tagged r{i} weigh {i*53 % 877} grams." for i in range(220))
system = [{"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}, {"type": "text", "text": "You are an agent."}]
tools = [{"name": "task", "description": "split work", "input_schema": {"type": "object", "properties": {"context": {"type": "string"}, "agents": {"type": "array", "items": {"type": "object"}}}}}]
base = [{"role": "user", "content": [{"type": "text", "text": parent_facts + "\nSplit this into two agents."}]},
        {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_probe1", "name": "task", "input": {"context": "(shared)", "agents": [{"name": "a"}, {"name": "b"}]}}]}]
def child(name, where):
    specific = {"type": "text", "text": f"You are {name}. Report only the weight of r{3 if name=='a' else 5}."}
    if where == "result+text":
        result = {"type": "tool_result", "tool_use_id": "toolu_probe1", "content": shared, "cache_control": {"type": "ephemeral"}}
        content = [result, specific]
    else:
        content = [{"type": "tool_result", "tool_use_id": "toolu_probe1", "content": [{"type": "text", "text": shared}, specific]}]
    body = {"model": "claude-sonnet-5", "max_tokens": 200, "system": system, "tools": tools, "messages": base + [{"role": "user", "content": content}], "cache_control": {"type": "ephemeral"}}
    return body
for where in sys.argv[2:]:
    for name in ["a", "b", "b"]:
        r, claim = call(child(name, where))
        u = r.get("usage", r)
        print(where, name, claim, {k: u.get(k) for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", "output_tokens")} if "usage" in r else r)
        time.sleep(2)
