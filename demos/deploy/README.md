# deploy: the home brain on Max's phone

The `home` brain (`dev/serve home 4320`) is the deployed one. Cloudflare's tunnel on this machine carries `harness.maxeonyx.com` to it, with Cloudflare Access in front, and Chrome on Android installs the face as an app.

`demos/deploy/run.sh` shows the face as the phone reaches it, on the scripted model, free, in about 30 s: a test brain behind `access.mjs`, a stand-in for the tunnel and Access that terminates TLS on another port and checks the brain's certificate as cloudflared does. On a phone-sized Chromium it prints Chrome's installability errors (none: `{"installabilityErrors":[]}`), then expires the session and shows the page leaving for the login (`302 GET /wiring`, `302 GET /`).

## The tunnel

`/etc/cloudflared/config.yml` already has the entry, above the final `http_status:404`:

```yaml
  - hostname: harness.maxeonyx.com
    service: https://localhost:4320
```

It needs no `noTLSVerify`: the brain serves `~/.config/dev-certs/shared-local/wildcard-local.crt`, which names `localhost`, and its CA is in the system's trust store, which cloudflared uses. `curl https://localhost:4320/wiring`, with no `-k`, answers 200. WebSockets (`/link`, `/dictation`) pass through the tunnel with no setting.

## Access

A self-hosted application for `harness.maxeonyx.com`, the whole domain with no path, and one Allow policy: Max's email. It already answers: an unauthenticated request gets a 302 to `shiny-union-cf1b.cloudflareaccess.com`.

- **Session duration:** long, say 1 month. Each expiry is a login on the phone. When one expires, the page notices at its next dropped link and reloads into the login.
- **Icons, if needed:** if the installed app's icon carries a small Chrome badge, Chrome installed a shortcut rather than an app, because Access turned away the fetch of its icon. Then add a second self-hosted application for the paths `harness.maxeonyx.com/icon.png` and `harness.maxeonyx.com/page/manifest.webmanifest`, with one Bypass policy for everyone. Neither holds anything private.

## Who else can reach the brain

The brain listens on every interface, but answers only this machine (so the tunnel) and the Windows host (the default route's gateway, so `https://mx-hv-mj.local:4320` from Edge). Every other peer on the network is dropped before TLS, so nothing on the LAN gets around Access.

## On the phone

Open https://harness.maxeonyx.com in Chrome, log in, then ⋮ → **Install app** (or **Add to home screen** → Install). It opens standalone, with no browser bar.

## After a reboot

`harness-home.service` is a transient unit (`systemd-run`), so it does not come back after a reboot: run `dev/serve home 4320` again from a clean clone, with the flags it ran with (`systemctl --user cat harness-home` shows them while it runs; today `--sample demos/home/sample.wav`).
