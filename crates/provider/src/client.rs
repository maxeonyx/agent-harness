//! Sending a request and reading its stream.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId};
use harness_core::id::{Timestamp, ValueId};
use harness_core::model_call::{Ending, ModelCall, Quota, Usage, Window};
use harness_runtime::{Cancel, Cancelled};

use crate::http::Response;
use crate::login::{Login, Token};
use crate::response::Assembly;
use crate::scripted::{self, SCRIPTED, scripted};
use crate::{Error, Model, TrustError, http, request};

const HOST: &str = "api.anthropic.com";
const PATH: &str = "/v1/messages";
const COUNT: &str = "/v1/messages/count_tokens";
/// What a request body says that only generating reads, which the token
/// count refuses.
const GENERATING: [&str; 8] = [
    "max_tokens",
    "stream",
    "temperature",
    "top_p",
    "top_k",
    "stop_sequences",
    "metadata",
    "service_tier",
];
const USAGE: &str = "/api/oauth/usage";
const MODELS: &str = "/v1/models?limit=1000";
const UNIFIED: &str = "anthropic-ratelimit-unified-";

/// The Messages API on the subscription login. Blocking, and shareable
/// across threads: each request is one call to [`Client::send`].
pub struct Client {
    tls: Arc<rustls::ClientConfig>,
    login: Login,
    /// How many times the scripted model has been asked each request, for
    /// its `flaky` and `vary`.
    seen: Mutex<HashMap<u64, u32>>,
}

/// What a request reports while it runs.
pub enum Progress {
    /// The response has started: its [`FirstByte`](harness_core::model_call::ModelCallEvent::FirstByte). The
    /// caller records [`Sent`](harness_core::model_call::ModelCallEvent::Sent) before sending, and the end
    /// is [`Client::send`]'s return value.
    FirstByte,
    /// Visible text as it streams.
    Text(String),
}

/// The provider's answer to a token count ([`Client::count`]).
pub enum Counted {
    /// It accepts the request as it is: this many input tokens.
    Tokens(u64),
    /// It refuses the request, in its own words.
    Refused { status: u16, message: String },
}

/// How a request ended: the fields of its [`ModelCallEvent::Ended`], and
/// the bytes its ending names.
pub struct Finished {
    pub ending: Ending,
    /// The bytes of [`Ending::Responded`]'s content or
    /// [`Ending::Cut`]'s partial, which travel in the batch that
    /// records the ending. The brain reads a response's tool uses from them.
    pub value: Option<Vec<u8>>,
    pub usage: Option<Usage>,
    pub quota: Option<Quota>,
}

impl Client {
    pub fn new(login: Login) -> Result<Client, TrustError> {
        Ok(Client {
            tls: http::tls()?,
            login,
            seen: Mutex::default(),
        })
    }

    /// The login every request is sent on.
    pub fn login(&self) -> &Login {
        &self.login
    }

    /// One exchange with the provider on the login: `method` on `path`
    /// (`/v1/models`) with `body`, and the headers every call carries. The
    /// response's head has arrived; its body streams. For anything the
    /// harness has no call for: probing an endpoint, a parameter, a model.
    pub fn exchange(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        cancel: &Cancel,
    ) -> Result<Response, Error> {
        let send = |token: &Token| {
            let bearer = token.bearer();
            let headers = [
                ("authorization", bearer.as_str()),
                ("anthropic-version", "2023-06-01"),
                ("anthropic-beta", "oauth-2025-04-20"),
                ("content-type", "application/json"),
                ("user-agent", concat!("harness/", env!("CARGO_PKG_VERSION"))),
            ];
            http::exchange(&self.tls, method, HOST, path, &headers, body, cancel).map_err(Error::Io)
        };
        let token = self.login.token()?;
        let response = send(&token)?;
        if response.status != 401 {
            return Ok(response);
        }
        // A token refused before its time: refreshed once, and sent again.
        let fresh = self.login.refused(&token)?;
        let mut response = send(&fresh)?;
        if response.status != 401 {
            return Ok(response);
        }
        let mut text = String::new();
        response.body.read_to_string(&mut text).map_err(Error::Io)?;
        Err(self
            .login
            .revoked(&fresh, &format!("401: {}", message(&text)))
            .into())
    }

    /// A GET of `path`, its body as JSON. `Err` says why there is none; it
    /// never contains the token.
    fn get(&self, path: &str, cancel: &Cancel) -> Result<serde_json::Value, String> {
        let mut response = self
            .exchange("GET", path, b"", cancel)
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
        Ok(parsed)
    }

    /// The models on offer: the provider's (`/v1/models`), newest first,
    /// then the [scripted](crate::SCRIPTED) one this client serves itself.
    pub fn models(&self, cancel: &Cancel) -> Result<Vec<Model>, String> {
        let listed = self.get(MODELS, cancel)?;
        let listed = listed["data"]
            .as_array()
            .ok_or("no `data` in the model list")?;
        Ok(listed
            .iter()
            .filter_map(|m| {
                let id = m["id"].as_str()?;
                Some(Model {
                    id: id.to_owned(),
                    name: m["display_name"].as_str().unwrap_or(id).to_owned(),
                })
            })
            .chain([scripted()])
            .collect())
    }

    /// Has the provider count the input tokens of `call`, as [`Client::send`]
    /// would send it, generating nothing: whether it accepts the request's
    /// shape, and if not, why. The [scripted](crate::SCRIPTED) model is
    /// counted by nobody: name a real one in `call.model`.
    ///
    /// `Err` means nothing was counted: the context, the login or the
    /// exchange is unusable.
    pub fn count(
        &self,
        call: &ModelCall,
        base: &[(EntryId, &Entry)],
        value: impl FnMut(ValueId) -> Option<Vec<u8>>,
        cancel: &Cancel,
    ) -> Result<Counted, Error> {
        let body = request::body(call, base, value)?;
        let mut body: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&body)
            .map_err(|e| Error::Request(request::RequestError(e.to_string())))?;
        for member in GENERATING {
            body.remove(member);
        }
        let body = serde_json::to_vec(&body).expect("a JSON object serialises");
        let mut response = self.exchange("POST", COUNT, &body, cancel)?;
        let mut text = String::new();
        response.body.read_to_string(&mut text).map_err(Error::Io)?;
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        Ok(match parsed["input_tokens"].as_u64() {
            Some(tokens) if response.status == 200 => Counted::Tokens(tokens),
            _ => Counted::Refused {
                status: response.status,
                message: parsed["error"]["message"]
                    .as_str()
                    .map_or(text.clone(), str::to_owned),
            },
        })
    }

    /// Streams the shortest answer `model` gives to a one-word request, on
    /// the path every request's response takes: whether the provider's
    /// stream still reads as this build expects.
    pub fn ping(&self, model: &str, cancel: &Cancel) -> Result<Finished, Error> {
        let body = serde_json::json!({
            "model": model,
            "max_tokens": 8,
            "stream": true,
            "system": [{"type": "text", "text": request::IDENTITY}],
            "messages": [{"role": "user", "content": "Say ok."}],
        });
        let body = serde_json::to_vec(&body).expect("JSON serialises");
        self.respond(&body, model == SCRIPTED, cancel, |_| {})
    }

    /// Sends `call`, built by [`request::body`] from `base` (the context it
    /// extends) and the bytes `value` looks up, and streams its response
    /// through `on`.
    ///
    /// `Err` means nothing was sent: the context or the login is unusable.
    /// Otherwise `on` sees [`Progress::FirstByte`] when `message_start`
    /// arrives, then the visible text as [`Progress::Text`].
    ///
    /// Requesting `cancel` cuts the request at once, wherever it is blocked:
    /// it ends [`Ending::Cut`] with the visible text so far and the
    /// usage the stream had reported. A request has no graceful phase; its
    /// owner decides whether to let it finish.
    pub fn send(
        &self,
        call: &ModelCall,
        base: &[(EntryId, &Entry)],
        value: impl FnMut(ValueId) -> Option<Vec<u8>>,
        cancel: &Cancel,
        on: impl FnMut(Progress),
    ) -> Result<Finished, Error> {
        let body = request::body(call, base, value)?;
        let local = request::model(call, base) == Some(SCRIPTED);
        self.respond(&body, local, cancel, on)
    }

    /// Sends `body`, to the scripted model if `local`, and reads its response.
    fn respond(
        &self,
        body: &[u8],
        local: bool,
        cancel: &Cancel,
        mut on: impl FnMut(Progress),
    ) -> Result<Finished, Error> {
        let end = |ending, usage, quota| Finished {
            ending,
            value: None,
            usage,
            quota,
        };
        let cut = |e: std::io::Error, status| match Cancelled::is(&e) {
            true => Ending::Cut { partial: None },
            false => failed(status, e.to_string(), None),
        };

        let response = match local {
            true => Ok(scripted::respond(body, cancel, &self.seen)),
            false => self.exchange("POST", PATH, body, cancel),
        };
        let response = match response {
            Ok(response) => response,
            Err(Error::Io(e)) => return Ok(end(cut(e, None), None, None)),
            Err(e) => return Err(e),
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
        // A response that reports no window (the scripted model's, a
        // refused login's) is no reading of the quota.
        let quota = Some(Quota {
            five_hour: window("5h"),
            seven_day: window("7d"),
        })
        .filter(|q| q.five_hour.is_some() || q.seven_day.is_some());
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
                return Ok(end(Ending::Cut { partial: None }, None, quota));
            }
            let ending = match limited {
                Some(until) => Ending::Limited { until },
                None => failed(Some(status), message(&text), retry),
            };
            return Ok(end(ending, None, quota));
        }
        let mut assembly = Assembly::default();
        let mut lines = BufReader::new(reader).lines();
        let ending = loop {
            let (name, data) = match next_event(&mut lines) {
                Some(Ok(event)) => event,
                Some(Err(e)) if Cancelled::is(&e) => {
                    let partial = assembly.visible();
                    return Ok(Finished {
                        ending: Ending::Cut {
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
                on(Progress::Text(chunk.to_owned()));
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

    /// The plan's usage, read from its own endpoint: each window it reports,
    /// by name (`five_hour`, `seven_day`, `seven_day_opus`, ...), as a
    /// fraction used and when it resets. A window it reports as `null` is
    /// left out, as is any field it adds. `Err` says why there is no
    /// reading; it never contains the token.
    pub fn usage(&self, cancel: &Cancel) -> Result<BTreeMap<String, Window>, String> {
        let parsed = self.get(USAGE, cancel)?;
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

/// The visible text an event adds, if it adds any.
fn text_delta<'a>(name: &str, data: &'a serde_json::Value) -> Option<&'a str> {
    let delta = &data["delta"];
    (name == "content_block_delta" && delta["type"] == "text_delta")
        .then(|| delta["text"].as_str())
        .flatten()
}

/// The provider's own words in an error body (`{"error": {"message"}}`),
/// else the body as it came.
fn message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_else(|| body.to_owned())
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
