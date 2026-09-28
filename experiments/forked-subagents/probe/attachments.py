"""Will Anthropic take a tool call the model did not make: an assistant tool_use turn with no thinking, then its result?"""
import json, sqlite3, time, urllib.request, os
cred = json.loads(sqlite3.connect(f"file:{os.path.expanduser('~/.local/share/opencode/opencode.db')}?mode=ro", uri=True)
    .execute("select value from credential where integration_id='anthropic' order by time_updated desc limit 1").fetchone()[0])
assert cred["expires"] / 1000 > time.time() + 120, "token expired"
def call(body):
    req = urllib.request.Request("https://api.anthropic.com/v1/messages", data=json.dumps(body).encode(), headers={
        "authorization": f"Bearer {cred['access']}", "anthropic-version": "2023-06-01", "anthropic-beta": "oauth-2025-04-20", "content-type": "application/json"})
    try:
        with urllib.request.urlopen(req) as r:
            b = json.load(r)
            return "200 " + json.dumps([{k: (v[:160] if isinstance(v, str) else v) for k, v in c.items() if k in ("type", "text", "thinking")} for c in b["content"]])
    except urllib.error.HTTPError as e:
        return f"{e.code} {e.read().decode()[:300]}"
system = [{"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}, {"type": "text", "text": "You are an agent."}]
tools = [{"name": "read_file", "description": "Read a file.", "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}}]
attached = {"role": "assistant", "content": [{"type": "tool_use", "id": "attach_0", "name": "read_file", "input": {"path": "notes/plan.md"}}]}
answer = {"type": "tool_result", "tool_use_id": "attach_0", "content": "The plan: ship the feed on Friday. The code word is TUATARA."}
for thinking in (None, {"type": "enabled", "budget_tokens": 2000}):
    body = {"model": "claude-sonnet-5", "max_tokens": 4000, "system": system, "tools": tools,
            "messages": [{"role": "user", "content": [{"type": "text", "text": "The plan file has been attached for you."}]},
                         attached,
                         {"role": "user", "content": [answer, {"type": "text", "text": "What is the code word in the plan? Answer in one line."}]}]}
    if thinking: body["thinking"] = thinking
    print("thinking", "on" if thinking else "default", "→", call(body))
