//! Server tool uses the provider will not answer, left out of a request.
//!
//! A server tool (`web_fetch`, `web_search`) runs inside a response: its
//! `server_tool_use` and its result are both blocks of the model's
//! message. When the model also makes a client tool use, the response can
//! stop for that before a server tool use has its result. The provider then
//! answers it at the start of the next response, but only when the request
//! ends with that message's tool results and nothing else: with any other
//! block after them it refuses the whole request (400, "tool use with id
//! ... was found without a corresponding ... result block"), and that
//! context could never be sent again. The record keeps the response as it
//! came; only what is sent leaves the unanswered use out.

use std::collections::HashSet;

use harness_core::context::Role;
use serde_json::Value;
use serde_json::value::RawValue;

/// `contents`, each message's content blocks in order with its role, with
/// every server tool use left out that has no result in any message and
/// that the provider will not answer next. The others' bytes are unchanged.
pub(crate) fn answerable(contents: &mut [(Role, Vec<u8>)]) {
    if !contents
        .iter()
        .any(|(_, c)| c.windows(USE.len()).any(|w| w == USE))
    {
        return;
    }
    let answered: HashSet<String> = contents
        .iter()
        .filter(|(role, _)| *role == Role::Assistant)
        .flat_map(|(_, c)| fields(c))
        .filter(|b| {
            b["type"]
                .as_str()
                .is_some_and(|t| t.ends_with("_tool_result"))
        })
        .filter_map(|b| b["tool_use_id"].as_str().map(String::from))
        .collect();
    // The last response, when only its tool results follow it: the
    // provider answers what it left unanswered.
    let last = contents
        .iter()
        .rposition(|(role, _)| *role == Role::Assistant);
    let pending = last.filter(|&i| {
        contents[i + 1..]
            .iter()
            .all(|(_, c)| fields(c).iter().all(|b| b["type"] == "tool_result"))
    });
    for (i, (role, content)) in contents.iter_mut().enumerate() {
        if *role != Role::Assistant || Some(i) == pending {
            continue;
        }
        let Ok(blocks) = serde_json::from_slice::<Vec<&RawValue>>(content) else {
            continue;
        };
        let kept: Vec<&str> = blocks
            .iter()
            .map(|b| b.get())
            .filter(|b| {
                let block: Value = serde_json::from_str(b).unwrap_or_default();
                block["type"] != "server_tool_use"
                    || block["id"].as_str().is_some_and(|id| answered.contains(id))
            })
            .collect();
        if kept.len() < blocks.len() {
            *content = format!("[{}]", kept.join(",")).into_bytes();
        }
    }
}

const USE: &[u8] = b"\"server_tool_use\"";

/// A message's content blocks as JSON values; none if it is not an array.
fn fields(content: &[u8]) -> Vec<Value> {
    serde_json::from_slice(content).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of the response that stopped design-graph at 21:51 on 31
    /// Oct 2026: the result of the previous response's fetch, which the
    /// provider answered first, then a fetch it never answered beside a
    /// client tool use.
    const RESPONSE: &str = r#"[{"type":"web_fetch_tool_result","tool_use_id":"srvtoolu_01DFYqBvKv5xsvArCPWEEbhx","content":{"type":"web_fetch_result","url":"https://developers.cloudflare.com/cache/how-to/edge-browser-cache-ttl/"},"caller":{"type":"direct"}},{"type":"thinking","thinking":"","signature":"CAQSqCgK"},{"type":"server_tool_use","id":"srvtoolu_01QxUrQfr9vHzaLbD1Lg9WPN","name":"web_fetch","input":{"url":"https://developers.cloudflare.com/cache/how-to/edge-browser-cache-ttl/set-browser-ttl/index.md"}},{"type":"tool_use","id":"toolu_01Tra1hXe2ktzKzynTcz1yQG","name":"bash","caller":{"type":"direct"},"input":{"command":"grep -rn cache-control crates/face-web/src"}}]"#;
    /// The response before it, whose fetch the provider did answer.
    const BEFORE: &str = r#"[{"type":"thinking","thinking":"","signature":"CAQSzw"},{"type":"server_tool_use","id":"srvtoolu_01DFYqBvKv5xsvArCPWEEbhx","name":"web_fetch","input":{"url":"https://developers.cloudflare.com/cache/how-to/edge-browser-cache-ttl/"}},{"type":"tool_use","id":"toolu_01KuSFyWUnTHySM5rooMXxHG","name":"bash","input":{"command":"ls"}}]"#;
    const RESULT: &str = r#"[{"type":"tool_result","tool_use_id":"toolu_01Tra1hXe2ktzKzynTcz1yQG","content":[{"type":"text","text":"no match"}]}]"#;
    const RESULT_AND_NOTES: &str = r#"[{"type":"tool_result","tool_use_id":"toolu_01Tra1hXe2ktzKzynTcz1yQG","content":[{"type":"text","text":"no match"}]},{"type":"text","text":"[harness] Sat 31 Oct 2026 21:51 NZDT."},{"type":"text","text":"[harness] Changed since you last saw them: FEEDBACK.md."}]"#;

    fn request(last: &str) -> Vec<(Role, Vec<u8>)> {
        let earlier = r#"[{"type":"tool_result","tool_use_id":"toolu_01KuSFyWUnTHySM5rooMXxHG","content":"ok"}]"#;
        [
            (Role::User, r#"[{"type":"text","text":"look"}]"#),
            (Role::Assistant, BEFORE),
            (Role::User, earlier),
            (Role::Assistant, RESPONSE),
            (Role::User, last),
        ]
        .into_iter()
        .map(|(r, c)| (r, c.as_bytes().to_vec()))
        .collect()
    }

    fn sent(contents: &[(Role, Vec<u8>)]) -> Vec<&str> {
        contents
            .iter()
            .map(|(_, c)| std::str::from_utf8(c).unwrap())
            .collect()
    }

    #[test]
    fn an_unanswered_fetch_before_more_than_tool_results_is_left_out() {
        let mut contents = request(RESULT_AND_NOTES);
        answerable(&mut contents);
        let sent = sent(&contents);
        assert!(!sent[3].contains("srvtoolu_01QxUrQfr9vHzaLbD1Lg9WPN"));
        assert_eq!(
            sent[3],
            RESPONSE.replace(
                r#"{"type":"server_tool_use","id":"srvtoolu_01QxUrQfr9vHzaLbD1Lg9WPN","name":"web_fetch","input":{"url":"https://developers.cloudflare.com/cache/how-to/edge-browser-cache-ttl/set-browser-ttl/index.md"}},"#,
                ""
            )
        );
        // The fetch answered in the next response is kept, with its result.
        assert_eq!(sent[1], BEFORE);
    }

    #[test]
    fn an_unanswered_fetch_before_only_its_tool_results_is_sent_for_the_provider_to_answer() {
        let mut contents = request(RESULT);
        answerable(&mut contents);
        assert_eq!(sent(&contents)[3], RESPONSE);
    }

    #[test]
    fn once_the_context_moves_on_the_unanswered_fetch_stays_left_out() {
        let mut contents = request(RESULT);
        contents.push((
            Role::Assistant,
            br#"[{"type":"text","text":"done"}]"#.to_vec(),
        ));
        answerable(&mut contents);
        assert!(!sent(&contents)[3].contains("srvtoolu_01QxUrQfr9vHzaLbD1Lg9WPN"));
    }
}
