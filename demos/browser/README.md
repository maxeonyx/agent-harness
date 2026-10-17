# browser: a chat hands a chore in real pages to a browser agent

```sh
demos/browser/run.sh                                   # the harness launches a headless browser
HARNESS_BROWSER=127.0.0.1:9222 demos/browser/run.sh    # or drives the one at this endpoint
```

It runs on the test brain `browser` (`dev/drive.mjs`) and prints its link to watch. It takes under a minute and about ten requests on the subscription, every role on Sonnet at low effort (`MODEL=…` for another).

What happens:

1. You ask a chat (the `chat` role) to do your October expense claim: the receipts are a table in `site/receipts.html`, and the claim is a form in `site/claim.html` whose lines are added by a button. Only the work receipts, and don't submit it.
2. The chat starts a `browser` agent with a brief. The agent opens tabs of its own by running the `tab` tool through `background`. It reads the receipts with `page` (JavaScript in the tab, returning text), goes to the claim form in the same tab, and fills the form with a `browser` script that clicks **Add line** and fills every field in one go, with no screenshots. It checks the result by reading the form back, and leaves **Submit** alone.
3. Its tab is a running background call, so handing it on is `pass`. The agent passes the tab to the chat and finishes. The chat, which has no browser tools, passes it to you. It is now yours: it shows under the chat as handed to Max.
4. The script reads the browser over DevTools as you would see it: the form filled in, and not submitted.
5. You stop the call (`act PORT stop CALL`, the Stop button on it in the face), which closes the tab. Then you press Done, and the chat writes its result.

Where the browser is, the limb's settings say, from `HARNESS_BROWSER` when the brain starts:
- **Unset or empty:** the limb launches Playwright's `chrome-headless-shell` with a profile of its own, in the limb's state (`~/.local/state/harness/limbs/<limb>/profile`). On a test brain that state is in `.drive.ignore/browser/state`.
- **`host:port`, `http://…` or `ws://…`:** it attaches to a browser already running there. For a Chrome you can see on Windows, start it with a dedicated profile (never your own; Chrome refuses remote debugging on the default one anyway): `chrome.exe --remote-debugging-port=9222 --user-data-dir=%LOCALAPPDATA%\harness-chrome`. Then tunnel it into the VM from Windows with `ssh -N -R 9222:127.0.0.1:9222 mx-hv-mj.local`, and start the brain with `HARNESS_BROWSER=127.0.0.1:9222`. A tab you open yourself in that window is yours: agents can work in it when told, and never close it.
- **Any other path:** a browser executable to launch.

Changing it is a new version of the limb's definition, and a running limb switches browsers from its next call on.

## Real output

A run at 04:33 NZDT on 2026-10-18 on Sonnet at low effort, with the headless shell the limb launched. Paths are shortened to `…/`; the brain's lines are cut at 170 characters, the rest at 200.

```
brain browser on https://mx-hv-mj.local:4405/ (pid 2427662; every role on sonnet/low)
log …/.drive.ignore/browser/brain.log
end it with: node dev/drive.mjs down --brain browser
== Max asks a chat, in the chat role:
Can you do my October expense claim? The receipts are at file://…/demos/browser/site/receipts.html and the claim form is at file://…/demos/browser/site/claim.html. Only the work ones, not the personal

    0.0s 2c87e04249b34225.1  limb browser defined
    0.0s efff045ee27ed1d0.1  environment browser defined
    0.0s efff045ee27ed1d0.1  role …/roles/browser.md changed
    0.1s 2c87e04249b34225.4  tab 3c89caa951447857 is Max's
    3.4s 2c87e04249b34225.11  Can you do my October expense's spawn made
    3.4s 2c87e04249b34225.11  Can you do my October expense: responded, ToolUse, 1 calls (4 in, 0 cached, 274 out)
    3.4s 2c87e04249b34225.12  Can you do my October expense's spawn dispatched
    3.4s 2c87e04249b34225.12  Can you do my October expense-claim created (browser)
    4.0s 2c87e04249b34225.13  browser connected
    4.0s c17b47c7a7a2f837.1  browser described: 4 tools
    5.8s 2c87e04249b34225.18  Can you do my October expense-claim: responded, ToolUse, 1 calls (4 in, 11278 cached, 144 out)
    5.8s 2c87e04249b34225.19  Can you do my October expense-claim's tab `r` made {"url":"file://…/demos/b
    6.0s 2c87e04249b34225.21  Can you do my October expense-claim's tab `r` dispatched
    7.1s 2c87e04249b34225.23  Can you do my October expense-claim's page made {"tab": "r", "script": "return document.body.innerText"}
    7.1s 2c87e04249b34225.23  Can you do my October expense-claim: responded, ToolUse, 1 calls (2 in, 11630 cached, 74 out)
    8.7s 2c87e04249b34225.28  Can you do my October expense-claim: responded, ToolUse, 1 calls (2 in, 11816 cached, 167 out)
    8.7s 2c87e04249b34225.29  Can you do my October expense-claim's tab `c` made {"url":"file://…/demos/b
    8.9s 2c87e04249b34225.31  Can you do my October expense-claim's tab `c` dispatched
   10.0s 2c87e04249b34225.33  Can you do my October expense-claim's page made {"tab": "c", "script": "return document.body.innerHTML"}
   10.0s 2c87e04249b34225.33  Can you do my October expense-claim: responded, ToolUse, 1 calls (2 in, 12169 cached, 76 out)
   13.0s 2c87e04249b34225.38  Can you do my October expense-claim's browser made {"script": "const t=tab('c');\nconst rows=[[\"2026-10-02\",\"Air New Zealand – WLG–AKL fl
   13.0s 2c87e04249b34225.38  Can you do my October expense-claim: responded, ToolUse, 2 calls (2 in, 12376 cached, 474 out)
   13.0s 2c87e04249b34225.39  Can you do my October expense-claim's browser dispatched
   13.1s c17b47c7a7a2f837.4  Can you do my October expense-claim's browser completed
   15.1s 2c87e04249b34225.45  Can you do my October expense-claim: responded, ToolUse, 2 calls (4 in, 11630 cached, 130 out)
   15.1s 2c87e04249b34225.46  Can you do my October expense-claim's tab `r` abandoned
   15.1s 2c87e04249b34225.47  Can you do my October expense-claim's tab `c` passed to Can you do my October expense: Filled October claim form, not submitted; Max can rev
   15.3s 2c87e04249b34225.49  Can you do my October expense-claim's tab `r` stopping, within 10s
   15.5s c17b47c7a7a2f837.5  Can you do my October expense-claim's tab `r` interrupted
   15.5s c17b47c7a7a2f837.6  tab ED6D1CEB75D521DC5D54B4CD5158D70E changed
   18.5s 2c87e04249b34225.53  Can you do my October expense-claim: responded, EndTurn, 0 calls (2 in, 11950 cached, 322 out)
   21.6s 2c87e04249b34225.56  Can you do my October expense-claim: responded, EndTurn, 0 calls (4 in, 12205 cached, 358 out)
   21.6s 2c87e04249b34225.57  Can you do my October expense's spawn completed
   24.4s 2c87e04249b34225.60  Can you do my October expense: responded, ToolUse, 1 calls (4 in, 9224 cached, 202 out)
   24.4s 2c87e04249b34225.61  Can you do my October expense-claim's tab `c` passed to Max: This is the filled October claim form, not submitted. Max should check it and s
   27.7s 2c87e04249b34225.64  Can you do my October expense: responded, EndTurn, 0 calls (2 in, 9985 cached, 325 out)


== What the browser shows, read over DevTools
  tab 974586A0  claim.html  "Expense claim"
    period: October 2026
    1. 2026-10-02 | Air New Zealand – WLG–AKL flight | 189.00
    2. 2026-10-02 | Uber – Airport to office | 42.50
    3. 2026-10-03 | Ibis Auckland – Hotel, 1 night | 164.00
    4. 2026-10-04 | Mojo – Coffee with client | 11.80
    5. 2026-10-09 | Noel Leeming – USB-C dock | 229.00
    6. 2026-10-14 | Wellington Parking – Parking, client visit | 18.00
    total: $654.30; not submitted

== Handed to Max: the tab, still open
  2c87e04249b34225.29.1  tab `c`

== Max has checked it and closes the tab: Stop on the call
  act: tab b1e72604baa540cf linked
  act: Max presses Stop on 2c87e04249b34225.29.1
included
  no tabs open

== Max presses Done, and the chat writes its result
  act: tab d174b9a2c06148aa linked
  act: Max presses Done
included
== the task is done; its result:

**Result**

- **Done:** A browser agent read the October receipts and filled in the claim form with 6 work receipts, totalling 654.30. It did not submit the form. I passed the form tab to Max to check and submit.
- **Known lines:** I only have four of the six:
  - 2026-10-02, Air New Zealand flight WLG–AKL, 189.00.
  - 2026-10-02, Uber from the airport to the office, 42.50.
  - 2026-10-03, Ibis Auckland, 1 night, 164.00.
  - 2026-10-04, Mojo coffee with a client, 11.80.
- **Unverified:**
  - The agent lost context when it was compacted, so I don't have the other two lines. One of them is dated 2026-10-09.
  - I don't know which receipts it excluded as personal, or whether it flagged any as ambiguous.
  - I did not re-check the form or the receipts page myself.
- **Left undone:** Max still has to compare the form against `receipts.html` and submit it. He should check for missing work receipts and for personal ones that got in. The Mojo coffee is the likelies
```

Attached to a browser of your own (`HARNESS_BROWSER`), its own first tab shows too: no call holds it, so it counts as Max's, and nothing closes it.
