# browser: a chat hands a chore in real pages to a browser agent

```sh
demos/browser/run.sh                                   # the harness launches a headless browser
HARNESS_BROWSER=127.0.0.1:9222 demos/browser/run.sh    # or drives the one at this endpoint
```

Watch it at https://mx-hv-mj.local:4342 (another port: `PORT=4400 demos/browser/run.sh`). It takes about a minute and a dozen requests on the subscription: Opus for the chat, Sonnet for the browser agent.

What happens:

1. You ask a chat (the `chat` role) to do your October expense claim: the receipts are a table in `site/receipts.html`, and the claim is a form in `site/claim.html` whose lines are added by a button. Only the work receipts, and don't submit it.
2. The chat starts a `browser` agent with a brief. The agent opens tabs of its own by running the `tab` tool through `background`. It reads the receipts with `page` (JavaScript in the tab, returning text), goes to the claim form in the same tab, and fills the form with a `browser` script that clicks **Add line** and fills every field in one go, with no screenshots. It checks the result by reading the form back, and leaves **Submit** alone.
3. Its tab is a running background call, so handing it on is `pass`. The agent passes the tab to the chat and finishes. The chat, which has no browser tools, passes it to you. It is now yours: it shows under the chat as handed to Max.
4. The script reads the browser over DevTools as you would see it: the form filled in, and not submitted.
5. You stop the call (`act PORT stop CALL`, the Stop button on it in the face), which closes the tab. Then you press Done, and the chat writes its result.

Where the browser is, the limb's settings say, from `HARNESS_BROWSER` when the brain starts:
- **Unset or empty:** the limb launches Playwright's `chrome-headless-shell` with a profile of its own, in the limb's state (`~/.local/state/harness/limbs/<limb>/profile`). Here the demo sets `XDG_STATE_HOME`, so that state stays in `target/browser-demo`.
- **`host:port`, `http://…` or `ws://…`:** it attaches to a browser already running there. For a Chrome you can see on Windows, start it with a dedicated profile (never your own; Chrome refuses remote debugging on the default one anyway): `chrome.exe --remote-debugging-port=9222 --user-data-dir=%LOCALAPPDATA%\harness-chrome`. Then tunnel it into the VM from Windows with `ssh -N -R 9222:127.0.0.1:9222 mx-hv-mj.local`, and start the brain with `HARNESS_BROWSER=127.0.0.1:9222`. A tab you open yourself in that window is yours: agents can work in it when told, and never close it.
- **Any other path:** a browser executable to launch.

Changing it is a new version of the limb's definition, and a running limb switches browsers from its next call on.

## Real output

A run at 03:38 NZDT on 2026-10-18, attached to a headless shell at `127.0.0.1:9444` (`HARNESS_BROWSER=127.0.0.1:9444 PORT=4410`). Lines are cut at 200 characters.

```
    8.3s 66e3ee87a1b2fa5e.12  chat-expense-claim created (browser)
    8.9s 66e3ee87a1b2fa5e.13  browser connected
    8.9s a54d46806ee94623.1  browser described: 4 tools
   10.8s 66e3ee87a1b2fa5e.19  chat-expense-claim's tab `claim` made {"url":"file:///…/demos/browser/site/receipts.html"}
   12.5s 66e3ee87a1b2fa5e.23  chat-expense-claim's page made {"tab": "claim", "script": "return document.body.innerText"}
   …
   22.8s 66e3ee87a1b2fa5e.34  chat-expense-claim's browser made {"script": "const t=tab(\"claim\");\nconst rows=[[\"2026-10-02\",\"Air New Zealand – WLG–AKL flight\",\"18
   23.0s 66e3ee87a1b2fa5e.36  chat-expense-claim's tab `claim` passed to chat: Filled-in claim form for Max to check and submit.
   23.4s a54d46806ee94623.5  chat-expense-claim's browser completed
   52.6s 66e3ee87a1b2fa5e.41  chat's spawn completed
   55.6s 66e3ee87a1b2fa5e.45  chat-expense-claim's tab `claim` passed to Max: The October claim form is filled in but not submitted. Max should check it and submit it him
   62.4s 66e3ee87a1b2fa5e.48  chat: responded, EndTurn, 0 calls (2 in, 10332 cached, 559 out)

== What the browser shows, read over DevTools
  tab 31141DEB  claim.html  "Expense claim"
    period: October 2026
    1. 2026-10-02 | Air New Zealand – WLG–AKL flight | 189.00
    2. 2026-10-02 | Uber – Airport to office | 42.50
    3. 2026-10-03 | Ibis Auckland – Hotel, 1 night | 164.00
    4. 2026-10-04 | Mojo – Coffee with client | 11.80
    5. 2026-10-09 | Noel Leeming – USB-C dock | 229.00
    6. 2026-10-14 | Wellington Parking – Parking, client visit | 18.00
    total: $654.30; not submitted
  tab 04CAA20F  about:blank  "about:blank"

== Handed to Max: the tab, still open
  66e3ee87a1b2fa5e.19.1  tab `claim`

== Max has checked it and closes the tab: Stop on the call
  act: Max presses Stop on 66e3ee87a1b2fa5e.19.1
included
  tab 04CAA20F  about:blank  "about:blank"

== Max presses Done, and the chat writes its result
== the task is done; its result:

I filled in your October expense claim with the six work receipts, totalling **$654.30 NZD**. I didn't submit it: it's open in the `claim` tab, which is yours to check and submit.

- **What went in:** the flight (189.00), the Uber (42.50), the hotel (164.00), the client coffee (11.80), the USB-C dock (229.00) and the parking (18.00). …
- **What was left out:** the two receipts marked personal, the groceries (96.20) and the cinema (21.50).
```

The `about:blank` tab is the attached browser's own first tab. No call holds it, so it counts as Max's, and nothing closed it.
