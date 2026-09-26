# Probe runs, 2026-09-24

`anthropic/claude-sonnet-5` via OpenRouter. Per request: prompt tokens, tokens read from cache, tokens written to cache, cost. Earlier runs (before `probe.py` printed this format) are summarised from their raw output.

## Pinned to one provider — 4 runs, all hit

| run | provider | parent | each child |
| --- | --- | --- | --- |
| 1 | amazon-bedrock | cold: 0 read, 7,709 written | 7,709 read, 186 written |
| 2 | amazon-bedrock | 7,709 read | 7,709 read, 150 written |
| 3 | google-vertex | cold: 0 read, 7,709 written | 7,709 read, 158 written |
| 4 | google-vertex | 7,709 read | 7,709 read, 192 written |

## Unrouted — 1 of 2 runs missed for both children

| run | parent | each child |
| --- | --- | --- |
| 1 | cold: 7,709 written, $0.0209 | 7,709 read, 186 written, ~$0.003 |
| 2 | 7,709 read, $0.0035 | **0 read, 7,755 written, $0.021** |

## Latest output

```text
$ python3 probe.py amazon-bedrock
parent   Amazon Bedrock   prompt=7711 cached=7709 written=0 cost=$0.0031 content=None
child 0  Amazon Bedrock   prompt=7883 cached=7709 written=172 cost=$0.0021 content='- k3 = 757\n- k5 = 595'
child 1  Amazon Bedrock   prompt=7883 cached=7709 written=172 cost=$0.0021 content='The value of key k5 is **595**.'

$ python3 probe.py 
parent   Amazon Bedrock   prompt=7711 cached=7709 written=0 cost=$0.0028 content=None
child 0  Amazon Bedrock   prompt=7861 cached=7859 written=0 cost=$0.0017 content='k3 = 757\nk5 = 595'
child 1  Amazon Bedrock   prompt=7861 cached=7859 written=0 cost=$0.0017 content='k5 = 595'

```

## Discipline

Every child was told `You are fork <i>. Report only <key>, then stop.` In the three runs whose answers were captured — unrouted run 1 and the two in the latest output — fork 0 (k3) reported both keys 3 times out of 3, and fork 1 (k5) reported both once out of 3.
