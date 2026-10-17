# browser: a chat hands a chore in real pages to a browser agent

```sh
demos/browser/run.sh                                   # the harness launches a headless browser
HARNESS_BROWSER=127.0.0.1:9222 demos/browser/run.sh    # or drives the one at this endpoint
```

Watch it at https://mx-hv-mj.local:4342 (another port: `PORT=4400 demos/browser/run.sh`). It takes under a minute and about ten requests on the subscription, every role on Sonnet at its own effort (`MODEL=…` for another).

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

A run at 04:05 NZDT on 2026-10-18 on Sonnet, with the headless shell the limb launched (`PORT=4376`). The brain's lines are cut at 170 characters, the rest at 200.

```
    7.9s e3876ab41512153b.12  Can you do my October expense-expense-claim created (browser)
    8.4s e3876ab41512153b.13  browser connected
    8.4s 32272afa869c115f.1  browser described: 4 tools
   10.5s e3876ab41512153b.18  Can you do my October expense-expense-claim: responded, ToolUse, 1 calls (4 in, 0 cached, 145 out)
   10.5s e3876ab41512153b.19  Can you do my October expense-expense-claim's tab `claim` made {"url":"file:///…
   10.7s e3876ab41512153b.21  Can you do my October expense-expense-claim's tab `claim` dispatched
   12.0s e3876ab41512153b.23  Can you do my October expense-expense-claim's page made {"tab": "claim", "script": "return document.body.innerText"}
   12.0s e3876ab41512153b.23  Can you do my October expense-expense-claim: responded, ToolUse, 1 calls (2 in, 11607 cached, 75 out)
   13.8s e3876ab41512153b.28  Can you do my October expense-expense-claim's browser made {"script": "const t=tab('claim');await t.goto('file:///…
   13.8s e3876ab41512153b.28  Can you do my October expense-expense-claim: responded, ToolUse, 1 calls (2 in, 11794 cached, 155 out)
   13.8s e3876ab41512153b.29  Can you do my October expense-expense-claim's browser dispatched
   13.8s 32272afa869c115f.3  tab 8C6A206F64F146E075A4D922B06712E6 changed
   13.9s 32272afa869c115f.4  Can you do my October expense-expense-claim's browser completed
   17.4s e3876ab41512153b.34  Can you do my October expense-expense-claim's browser made {"script": "const t=tab('claim');\nconst r=[['2026-10-02','Air New Zealand – WLG–
   17.4s e3876ab41512153b.34  Can you do my October expense-expense-claim: responded, ToolUse, 2 calls (2 in, 12148 cached, 536 out)
   17.4s e3876ab41512153b.35  Can you do my October expense-expense-claim's browser dispatched
   17.4s e3876ab41512153b.36  Can you do my October expense-expense-claim's tab `claim` passed to Can you do my October expense: Filled claim form for Max to check and su
   17.5s 32272afa869c115f.5  Can you do my October expense-expense-claim's browser completed
   21.1s e3876ab41512153b.40  Can you do my October expense-expense-claim: responded, EndTurn, 0 calls (2 in, 12809 cached, 420 out)
   21.1s e3876ab41512153b.41  Can you do my October expense's spawn completed
   23.0s e3876ab41512153b.44  Can you do my October expense: responded, ToolUse, 1 calls (4 in, 8821 cached, 104 out)
   23.0s e3876ab41512153b.45  Can you do my October expense-expense-claim's tab `claim` passed to Max: This tab holds the filled-in October claim form. Max should check i
   26.6s e3876ab41512153b.48  Can you do my October expense: responded, EndTurn, 0 calls (2 in, 10393 cached, 429 out)


== What the browser shows, read over DevTools
  tab 8C6A206F  claim.html  "Expense claim"
    period: October 2026
    1. 2026-10-02 | Air New Zealand – WLG–AKL flight | 189.00
    2. 2026-10-02 | Uber – Airport to office | 42.50
    3. 2026-10-03 | Ibis Auckland – Hotel, 1 night | 164.00
    4. 2026-10-04 | Mojo – Coffee with client | 11.80
    5. 2026-10-09 | Noel Leeming – USB-C dock | 229.00
    6. 2026-10-14 | Wellington Parking – Parking, client visit | 18.00
    total: $654.30; not submitted

== Handed to Max: the tab, still open
  e3876ab41512153b.19.1  tab `claim`

== Max has checked it and closes the tab: Stop on the call
  act: tab 9f1348113ee29650 linked
  act: Max presses Stop on e3876ab41512153b.19.1
included
  no tabs open

== Max presses Done, and the chat writes its result
  act: tab 49ff5002d08cfadf linked
  act: Max presses Done
included
== the task is done; its result:

**Result**

**Done:** A browser agent filled in Max's October 2026 expense claim with the six work receipts, totalling $654.30 NZD. That matches the sum of the receipts. It didn't submit the form. I passed its ta

**Excluded:** The agent left out two receipts marked personal on the receipts page. These were Countdown groceries ($96.20) and Event Cinemas ($21.50).

**Learned:** The agent found no ambiguous receipts. All eight were dated October, had an amount, and none looked like a duplicate. I couldn't open the file URLs myself, so everything above comes from 

**Left undone:** Max still has to check the form and click Submit himself. I haven't done that.
```

Attached to a browser of your own (`HARNESS_BROWSER`), its own first tab shows too: no call holds it, so it counts as Max's, and nothing closes it.
