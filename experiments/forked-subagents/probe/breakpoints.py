"""How many explicit cache breakpoints does Anthropic take beside the automatic one?"""
import json, sqlite3, time, urllib.request, os
cred = json.loads(sqlite3.connect(f"file:{os.path.expanduser('~/.local/share/opencode/opencode.db')}?mode=ro", uri=True)
    .execute("select value from credential where integration_id='anthropic' order by time_updated desc limit 1").fetchone()[0])
assert cred["expires"] / 1000 > time.time() + 120, "token expired"
def call(body):
    req = urllib.request.Request("https://api.anthropic.com/v1/messages", data=json.dumps(body).encode(), headers={
        "authorization": f"Bearer {cred['access']}", "anthropic-version": "2023-06-01", "anthropic-beta": "oauth-2025-04-20", "content-type": "application/json"})
    try:
        with urllib.request.urlopen(req) as r:
            return "200 " + json.dumps(json.load(r)["usage"])
    except urllib.error.HTTPError as e:
        return f"{e.code} {e.read().decode()[:300]}"
system = [{"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}]
for explicit in (3, 4):
    for automatic in (False, True):
        blocks = [{"type": "text", "text": f"Part {i}.", "cache_control": {"type": "ephemeral"}} for i in range(explicit)]
        blocks.append({"type": "text", "text": "Say ok."})
        body = {"model": "claude-sonnet-5", "max_tokens": 20, "system": system, "messages": [{"role": "user", "content": blocks}]}
        if automatic:
            body["cache_control"] = {"type": "ephemeral"}
        print(f"explicit {explicit}, automatic {automatic}:", call(body))
