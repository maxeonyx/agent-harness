"""Fork a parent at its `task` call and fire two children in parallel, printing each request's cache usage.

Usage: python3 probe.py [provider-slug]   e.g. amazon-bedrock, google-vertex; omit to let OpenRouter route.
Reads OPENROUTER_API_KEY from ../keys.ignore.env.
"""

import concurrent.futures
import json
import pathlib
import sys
import urllib.request

KEYS = pathlib.Path(__file__).parent.parent / "keys.ignore.env"
API_KEY = dict(line.split("=", 1) for line in KEYS.read_text().split())["OPENROUTER_API_KEY"]
MODEL = "anthropic/claude-sonnet-5"
PROVIDER = {"order": [sys.argv[1]], "allow_fallbacks": False} if len(sys.argv) > 1 else None

SYSTEM = "You are a careful assistant.\n" + "\n".join(f"Fact {i}: the value of key k{i} is {i * 7919 % 1000}." for i in range(400))
TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "task",
            "description": "Launch subagents",
            "parameters": {
                "type": "object",
                "properties": {"agents": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "task": {"type": "string"}}}}},
                "required": ["agents"],
            },
        },
    }
]
PARENT = [
    {"role": "system", "content": SYSTEM},
    {"role": "user", "content": "Use the task tool to launch two agents: one to report k3, one to report k5. Call the tool now."},
]


def complete(messages):
    body = {"model": MODEL, "messages": messages, "tools": TOOLS, "max_tokens": 200, "cache_control": {"type": "ephemeral"}, "provider": PROVIDER}
    request = urllib.request.Request(
        "https://openrouter.ai/api/v1/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
    )
    return json.load(urllib.request.urlopen(request))


def report(label, response):
    usage = response["usage"]
    details = usage["prompt_tokens_details"]
    content = response["choices"][0]["message"].get("content")
    print(f"{label:8} {response.get('provider'):16} prompt={usage['prompt_tokens']} cached={details['cached_tokens']} written={details['cache_write_tokens']} cost=${usage['cost']:.4f} content={content!r}")


parent = complete(PARENT)
report("parent", parent)
message = parent["choices"][0]["message"]
call = message["tool_calls"][0]
turn = {"role": "assistant", "content": message.get("content") or "", "tool_calls": message["tool_calls"]}
if message.get("reasoning_details"):
    turn["reasoning_details"] = message["reasoning_details"]


def fork(index):
    key = ("k3", "k5")[index]
    tail = {"role": "tool", "tool_call_id": call["id"], "content": f"You are fork {index}. Report only {key}, then stop."}
    return complete(PARENT + [turn, tail])


with concurrent.futures.ThreadPoolExecutor(2) as pool:
    for index, child in enumerate(pool.map(fork, (0, 1))):
        report(f"child {index}", child)
