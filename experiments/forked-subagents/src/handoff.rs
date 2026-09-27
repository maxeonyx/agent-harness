//! A handoff: what one context hands the next.
//!
//! Down, a `task` call hands each child the part every sibling shares and
//! then its own. Up, the children's reports are the answer to the parent's
//! call. Across, an agent that hands over starts again from the system prompt
//! and what it wrote for itself. All three are delivered by `deliver`.

use crate::wire::{Message, ToolCall};

pub struct Handoff {
    /// The same bytes for every receiver of it.
    pub shared: String,
    /// Files read when the handoff was made. The receiver is shown them as
    /// `read_file` calls it did not make, after the shared part.
    pub attachments: Vec<Attachment>,
    /// This receiver's own part, as user text. A child reads an own part in
    /// a tool result as a broken call, so it never goes in one. The parent
    /// has none: its handoff answers its call, and its turn carries on.
    pub own: Option<String>,
    /// A cache breakpoint after everything the receivers share. Worth a slot
    /// only when there are siblings to share it.
    pub breakpoint: bool,
}

#[derive(Clone)]
pub struct Attachment {
    pub path: String,
    /// What `read_file` answered, error included.
    pub content: String,
}

/// A turn whose calls are answered except one: the one the handoff answers.
pub struct Pending<'a> {
    pub calls: &'a [ToolCall],
    pub call_index: usize,
    pub results: &'a [Option<Message>],
}

impl Handoff {
    /// `context` with this handoff delivered after it. With a pending call,
    /// the shared part is that call's answer, among the turn's other results;
    /// without one, it is user text.
    pub fn deliver(&self, mut context: Vec<Message>, answering: Option<Pending>) -> Vec<Message> {
        match answering {
            Some(turn) => {
                for (i, call) in turn.calls.iter().enumerate() {
                    context.push(if i == turn.call_index {
                        Message::tool_result(&call.id, &self.shared)
                    } else {
                        turn.results[i]
                            .clone()
                            .expect("only the handoff's own slot is unanswered")
                    });
                }
            }
            None => context.push(Message::new("user", &self.shared)),
        }
        if !self.attachments.is_empty() {
            // Ids are unique within a context: a child can be handed
            // attachments on top of those its parent was.
            let at = context.len();
            let ids: Vec<String> = (0..self.attachments.len())
                .map(|n| format!("attached_{at}_{n}"))
                .collect();
            let mut calls = Message::new("assistant", "");
            calls.tool_calls = Some(
                self.attachments
                    .iter()
                    .zip(&ids)
                    .map(|(attachment, id)| ToolCall::read_file(id, &attachment.path))
                    .collect(),
            );
            context.push(calls);
            for (attachment, id) in self.attachments.iter().zip(&ids) {
                context.push(Message::tool_result(id, &attachment.content));
            }
        }
        if self.breakpoint {
            context
                .last_mut()
                .expect("the shared part was just delivered")
                .cache = true;
        }
        if let Some(own) = &self.own {
            context.push(Message::new("user", own));
        }
        context
    }
}
