//! Sending a request and reading its stream.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId};
use harness_core::id::{ModelCallId, Timestamp, ValueId};
use harness_core::live::Live;
use harness_core::model_call::{Ending, ModelCall, Quota, Usage, Window};
use harness_runtime::{Cancel, Cancelled};

use crate::login::Login;
use crate::response::Assembly;
use crate::{Error, http, request};

const HOST: &str = "api.anthropic.com";
const PATH: &str = "/v1/messages";
const USAGE: &str = "/api/oauth/usage";
const UNIFIED: &str = "anthropic-ratelimit-unified-";

/// The Messages API on the subscription login. Blocking, and shareable
/// across threads: each request is one call to [`Client::send`].
pub struct Client {
    tls: Arc<rustls::ClientConfig>,
    login: Login,
}

/// What a request reports while it runs.
pub enum Progress {
    /// The response has started: its [`FirstByte`](harness_core::model_call::ModelCallEvent::FirstByte). The
    /// caller records [`Sent`](harness_core::model_call::ModelCallEvent::Sent) before sending, and the end
    /// is [`Client::send`]'s return value.
    FirstByte,
    /// Visible text as it streams.
    Live(Live),
}

/// How a request ended: the fields of its [`ModelCallEvent::Ended`], and
/// the bytes its ending names.
pub struct Finished {
    pub ending: Ending,
    /// The bytes of [`Ending::Responded`]'s content or
    /// [`Ending::Abandoned`]'s partial, which travel in the batch that
    /// records the ending. The brain reads a response's tool uses from them.
    pub value: Option<Vec<u8>>,
    pub usage: Option<Usage>,
    pub quota: Option<Quota>,
}

impl Client {
    pub fn new(login: Login) -> Client {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("ring supports the default protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
        Client {
            tls: Arc::new(tls),
            login,
        }
    }

    /// Sends `call`, built by [`request::body`] from `base` (the context it
    /// extends) and the bytes `value` looks up, and streams its response
    /// through `on`.
    ///
    /// `Err` means nothing was sent: the context or the login is unusable.
    /// Otherwise `on` sees [`Progress::FirstByte`] when `message_start`
    /// arrives, then the visible text as [`Live`].
    ///
    /// Requesting `cancel` cuts the request at once, wherever it is blocked:
    /// it ends [`Ending::Abandoned`] with the visible text so far and the
    /// usage the stream had reported. A request has no graceful phase; its
    /// owner decides whether to let it finish.
    ///
    /// A billing claim other than the plan's window is reported as
    /// [`Quota::on_plan`] `false`, and is a failure: the response is not read.
    pub fn send(
        &self,
        id: ModelCallId,
        call: &ModelCall,
        base: &[(EntryId, &Entry)],
        value: impl FnMut(ValueId) -> Option<Vec<u8>>,
        cancel: &Cancel,
        mut on: impl FnMut(Progress),
    ) -> Result<Finished, Error> {
        let body = request::body(call, base, value)?;
        let token = self.login.token()?;
        let end = |ending, usage, quota| Finished {
            ending,
            value: None,
            usage,
            quota,
        };
        let cut = |e: std::io::Error, status| match Cancelled::is(&e) {
            true => Ending::Abandoned { partial: None },
            false => failed(status, e.to_string(), None),
        };

        let bearer = token.bearer();
        let headers = [
            ("authorization", bearer.as_str()),
            ("anthropic-version", "2023-06-01"),
            ("anthropic-beta", "oauth-2025-04-20"),
            ("content-type", "application/json"),
            ("user-agent", concat!("harness/", env!("CARGO_PKG_VERSION"))),
        ];
        let response = match http::exchange(&self.tls, "POST", HOST, PATH, &headers, &body, cancel)
        {
            Ok(response) => response,
            Err(e) => return Ok(end(cut(e, None), None, None)),
        };
        let status = response.status;
        let header = |name: &str| response.header(name).map(str::to_owned);
        let ratio = |name: &str| header(&format!("{UNIFIED}{name}")).and_then(|v| v.parse().ok());
        let reset = |name: &str| {
            header(&format!("{UNIFIED}{name}-reset"))
                .and_then(|v| v.parse::<i64>().ok())
                .map(|s| Timestamp(s * 1000))
        };
        let window = |name: &str| {
            ratio(&format!("{name}-utilization")).map(|used| Window {
                used,
                resets: reset(name),
            })
        };
        let claim = header(&format!("{UNIFIED}representative-claim"));
        let on_plan = on_plan(claim.as_deref());
        let quota = Some(Quota {
            on_plan,
            five_hour: window("5h"),
            seven_day: window("7d"),
        });
        let retry = header("retry-after")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| Timestamp(now_ms() + (s * 1000.0) as i64));
        let limited = (status == 429
            && header(&format!("{UNIFIED}status")).as_deref() == Some("rejected"))
        .then(|| header(&format!("{UNIFIED}reset"))?.parse::<i64>().ok())
        .flatten()
        .map(|s| Timestamp(s * 1000));

        let mut reader = response.body;
        if status != 200 {
            let mut text = String::new();
            if let Err(e) = reader.read_to_string(&mut text)
                && Cancelled::is(&e)
            {
                return Ok(end(Ending::Abandoned { partial: None }, None, quota));
            }
            let message = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                .unwrap_or(text);
            let ending = match limited {
                Some(until) => Ending::Limited { until },
                None => failed(Some(status), message, retry),
            };
            return Ok(end(ending, None, quota));
        }
        if !on_plan {
            let message =
                format!("billed as {claim:?}, not to the plan; the response was not read");
            return Ok(end(failed(Some(status), message, None), None, quota));
        }

        let mut assembly = Assembly::default();
        let mut lines = BufReader::new(reader).lines();
        let ending = loop {
            let (name, data) = match next_event(&mut lines) {
                Some(Ok(event)) => event,
                Some(Err(e)) if Cancelled::is(&e) => {
                    let partial = assembly.visible();
                    return Ok(Finished {
                        ending: Ending::Abandoned {
                            partial: partial.as_deref().map(ValueId::of),
                        },
                        value: partial,
                        usage: assembly.usage(),
                        quota,
                    });
                }
                Some(Err(e)) => break cut(e, Some(status)),
                None => break failed(Some(status), "the stream ended early".into(), None),
            };
            let parsed: serde_json::Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(e) => break failed(Some(status), format!("bad event data: {e}"), None),
            };
            if name == "error" {
                let message = parsed["error"]["message"].as_str().unwrap_or(&data);
                break failed(Some(status), message.to_owned(), None);
            }
            if let Err(message) = assembly.apply(&name, &parsed) {
                break failed(Some(status), message, None);
            }
            if name == "message_start" {
                on(Progress::FirstByte);
            }
            if let Some(chunk) = text_delta(&name, &parsed) {
                on(Progress::Live(Live::Model {
                    call: id,
                    chunk: chunk.to_owned(),
                }));
            }
            if name == "message_stop" {
                let response = assembly.content().and_then(|content| {
                    Ok((content, assembly.stop.clone().ok_or("no stop_reason")?))
                });
                let (content, stop) = match response {
                    Ok(response) => response,
                    Err(message) => break failed(Some(status), message, None),
                };
                return Ok(Finished {
                    ending: Ending::Responded {
                        content: ValueId::of(&content),
                        stop,
                    },
                    value: Some(content),
                    usage: assembly.usage(),
                    quota,
                });
            }
        };
        Ok(end(ending, assembly.usage(), quota))
    }
}

impl Client {
    /// The plan's usage, read from its own endpoint: each window it reports,
    /// by name (`five_hour`, `seven_day`, `seven_day_opus`, ...), as a
    /// fraction used and when it resets. A window it reports as `null` is
    /// left out, as is any field it adds. `Err` says why there is no
    /// reading; it never contains the token.
    pub fn usage(&self, cancel: &Cancel) -> Result<BTreeMap<String, Window>, String> {
        let token = self.login.token().map_err(|e| e.to_string())?;
        let bearer = token.bearer();
        let headers = [
            ("authorization", bearer.as_str()),
            ("anthropic-beta", "oauth-2025-04-20"),
            ("user-agent", concat!("harness/", env!("CARGO_PKG_VERSION"))),
        ];
        let mut response = http::exchange(&self.tls, "GET", HOST, USAGE, &headers, b"", cancel)
            .map_err(|e| e.to_string())?;
        let mut text = String::new();
        response
            .body
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .map_err(|_| format!("{}: not JSON: {text:.200}", response.status))?;
        if response.status != 200 {
            let message = parsed["error"]["message"].as_str().unwrap_or(&text);
            return Err(format!("{}: {message:.200}", response.status));
        }
        let windows = parsed.as_object().into_iter().flatten();
        Ok(windows
            .filter_map(|(name, w)| {
                let used = w["utilization"].as_f64()? / 100.0;
                let resets = w["resets_at"].as_str().and_then(iso_ms).map(Timestamp);
                Some((name.clone(), Window { used, resets }))
            })
            .collect())
    }
}

/// Milliseconds since the epoch of an RFC 3339 time
/// (`2026-10-17T05:00:00.123+00:00`, or ending `Z`).
fn iso_ms(s: &str) -> Option<i64> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let rest = s.get(19..)?;
    let zone = rest.find(['Z', 'z', '+', '-'])?;
    let millis = match rest[..zone].strip_prefix('.') {
        Some(f) => format!("{f:0<3}").get(..3)?.parse::<i64>().ok()?,
        None => 0,
    };
    let offset = match &rest[zone..] {
        "Z" | "z" => 0,
        o => {
            let sign = if o.starts_with('-') { -1 } else { 1 };
            let oh: i64 = o.get(1..3)?.parse().ok()?;
            let om: i64 = o.get(4..6)?.parse().ok()?;
            sign * (oh * 60 + om)
        }
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((mo + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + h * 3600 + mi * 60 + sec - offset * 60;
    Some(seconds * 1000 + millis)
}

/// A plan window (`five_hour`, `seven_day`, ...). Anything else, including
/// no claim at all, is billing gone wrong.
fn on_plan(claim: Option<&str>) -> bool {
    claim.is_some_and(|c| c == "five_hour" || c.starts_with("seven_day"))
}

/// The visible text an event adds, if it adds any.
fn text_delta<'a>(name: &str, data: &'a serde_json::Value) -> Option<&'a str> {
    let delta = &data["delta"];
    (name == "content_block_delta" && delta["type"] == "text_delta")
        .then(|| delta["text"].as_str())
        .flatten()
}

fn failed(status: Option<u16>, message: String, retry: Option<Timestamp>) -> Ending {
    Ending::Failed {
        status,
        message,
        retry,
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The next server-sent event's name and raw data.
fn next_event(
    lines: &mut impl Iterator<Item = std::io::Result<String>>,
) -> Option<std::io::Result<(String, String)>> {
    let (mut name, mut data) = (String::new(), String::new());
    loop {
        let line = match lines.next() {
            None if data.is_empty() => return None,
            None => String::new(),
            Some(Err(e)) => return Some(Err(e)),
            Some(Ok(line)) => line,
        };
        if line.is_empty() {
            if data.is_empty() {
                continue;
            }
            return Some(Ok((name, data)));
        }
        if let Some(v) = line.strip_prefix("event:") {
            name = v.trim().to_owned();
        } else if let Some(v) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(v.strip_prefix(' ').unwrap_or(v));
        }
    }
}
