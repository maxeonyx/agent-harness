//! Sending a request and reading its stream.

use std::io::{BufRead, BufReader, Read};
use std::ops::ControlFlow;
use std::time::{SystemTime, UNIX_EPOCH};

use harness_core::context::{Entry, EntryId, Message, Role, Wire};
use harness_core::id::{CallId, ModelCallId, Timestamp};
use harness_core::live::Live;
use harness_core::model_call::{Ending, ModelCall, ModelCallEvent, Quota, Stop, Usage};

use crate::login::Login;
use crate::response::Assembly;
use crate::{Error, request};

const URL: &str = "https://api.anthropic.com/v1/messages";
const UNIFIED: &str = "anthropic-ratelimit-unified-";

/// The Messages API on the subscription login. Blocking, and shareable
/// across threads: each request is one call to [`Client::send`].
pub struct Client {
    agent: ureq::Agent,
    login: Login,
}

/// What a request reports while it runs.
pub enum Progress {
    /// [`ModelCallEvent::Sent`] or [`ModelCallEvent::FirstByte`], when it
    /// happens. The end is [`Client::send`]'s return value.
    Fact(ModelCallEvent),
    /// Each streamed event, as its raw data.
    Live(Live),
}

/// How a request ended, before the brain records it.
pub struct Finished {
    /// The response, or every other [`Ending`].
    pub result: Result<Reply, Ending>,
    pub usage: Option<Usage>,
    pub quota: Option<Quota>,
}

/// A complete response.
pub struct Reply {
    /// The assistant message after the request's head. The brain stores it
    /// as an [`Entry::Message`], records a call for each tool use in it, then
    /// records [`Reply::ending`].
    pub message: Message,
    pub stop: Stop,
}

impl Reply {
    /// [`Ending::Responded`], once `calls` (one per tool use, in order) are
    /// recorded.
    pub fn ending(&self, calls: Vec<CallId>) -> Ending {
        Ending::Responded {
            entry: Entry::Message(self.message.clone()).id(),
            stop: self.stop.clone(),
            calls,
        }
    }
}

impl Client {
    pub fn new(login: Login) -> Client {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        Client { agent, login }
    }

    /// Sends `call`, built from the entries `entry` looks up, and streams
    /// its response through `on`.
    ///
    /// `Err` means nothing was sent: the context or the login is unusable.
    /// Otherwise `on` sees [`ModelCallEvent::Sent`] first, then FirstByte
    /// when `message_start` arrives, and every event as [`Live`]. `on`
    /// returning `Break` stops reading, and the request ends
    /// [`Ending::Abandoned`] with the visible text so far.
    ///
    /// A billing claim other than the plan's window is a failure: the
    /// response is not read.
    pub fn send(
        &self,
        id: ModelCallId,
        call: &ModelCall,
        entry: impl FnMut(EntryId) -> Option<Entry>,
        mut on: impl FnMut(Progress) -> ControlFlow<()>,
    ) -> Result<Finished, Error> {
        let body = request::body(call, entry)?;
        let token = self.login.token()?;
        let end = |result: Result<Reply, Ending>, usage, quota| Finished {
            result,
            usage,
            quota,
        };
        if on(Progress::Fact(ModelCallEvent::Sent(call.clone()))).is_break() {
            return Ok(end(Err(Ending::Abandoned { partial: None }), None, None));
        }

        let response = match self
            .agent
            .post(URL)
            .header("authorization", &token.bearer())
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("content-type", "application/json")
            .send(body.as_bytes())
        {
            Ok(response) => response,
            Err(e) => return Ok(end(Err(failed(None, e.to_string(), None)), None, None)),
        };
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let ratio = |name: &str| header(&format!("{UNIFIED}{name}")).and_then(|v| v.parse().ok());
        let quota = Some(Quota {
            claim: header(&format!("{UNIFIED}representative-claim")),
            five_hour: ratio("5h-utilization"),
            seven_day: ratio("7d-utilization"),
        });
        let retry = header("retry-after")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| Timestamp(now_ms() + (s * 1000.0) as i64))
            .or_else(|| {
                (status == 429)
                    .then(|| header(&format!("{UNIFIED}reset"))?.parse::<i64>().ok())
                    .flatten()
                    .map(|s| Timestamp(s * 1000))
            });

        let mut reader = response.into_body().into_reader();
        if status != 200 {
            let mut text = String::new();
            let _ = reader.read_to_string(&mut text);
            let message = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                .unwrap_or(text);
            return Ok(end(Err(failed(Some(status), message, retry)), None, quota));
        }
        let claim = quota.as_ref().and_then(|q| q.claim.as_deref());
        if !on_plan(claim) {
            let message =
                format!("billed as {claim:?}, not to the plan; the response was not read");
            return Ok(end(Err(failed(Some(status), message, None)), None, quota));
        }

        let mut assembly = Assembly::default();
        let mut lines = BufReader::new(reader).lines();
        let ending = loop {
            let (name, data) = match next_event(&mut lines) {
                Some(Ok(event)) => event,
                Some(Err(e)) => break failed(None, e.to_string(), None),
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
            if name == "message_start" && on(Progress::Fact(ModelCallEvent::FirstByte)).is_break() {
                break Ending::Abandoned { partial: None };
            }
            let live = Live::Model {
                call: id,
                event: Wire(data),
            };
            if on(Progress::Live(live)).is_break() {
                break Ending::Abandoned {
                    partial: assembly.visible(),
                };
            }
            if name == "message_stop" {
                let reply = assembly.content().and_then(|content| {
                    let stop = assembly.stop.clone().ok_or("no stop_reason")?;
                    let message = Message {
                        parent: call.head,
                        role: Role::Assistant,
                        content,
                    };
                    Ok(Reply { message, stop })
                });
                let result = reply.map_err(|message| failed(Some(status), message, None));
                return Ok(end(result, assembly.usage(), quota));
            }
        };
        Ok(end(Err(ending), assembly.usage(), quota))
    }
}

/// A plan window (`five_hour`, `seven_day`, ...). Anything else, including
/// no claim at all, is billing gone wrong.
fn on_plan(claim: Option<&str>) -> bool {
    claim.is_some_and(|c| c == "five_hour" || c.starts_with("seven_day"))
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
