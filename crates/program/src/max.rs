//! The Max limb: `ask`, a question Max answers from his queue.
//!
//! Its calls are never dispatched. His face shows each one in his queue, and
//! his answer is the settlement he authors. Asked in the round, the asker
//! waits for him; asked through `background`, it carries on with what it
//! thinks best and his answer arrives as a message when he gives it.

use harness_core::environment::{Audience, Description, Instruction, Limb, LimbKind, Tool};
use harness_core::id::{Digest, LimbId};
use harness_core::resource::{Observation, ResourceKey, Version};
use serde_json::json;

pub const ASK: &str = "ask";

pub fn definition(id: LimbId) -> Limb {
    Limb {
        id,
        name: "max".into(),
        kind: LimbKind::Max,
    }
}

pub fn description(limb: LimbId) -> Description {
    let said = String::from(
        "`ask` puts a question in Max's queue. Ask only what Max alone knows: what he wants, how consequences rank, \
a choice between materially different shapes. Say what you would do and why, so he can answer in a word.\n\n\
If you can carry on without his answer, make the choice you think best and ask in the background \
(`background` with `{\"tool\": \"ask\", \"input\": {...}}`): his answer reaches you when he gives it, and you \
cannot finish until he has. Ask directly only when nothing useful can happen before he answers.",
    );
    let ask = json!({
        "name": ASK,
        "description": "Ask Max a question, and wait for his answer.",
        "input_schema": {
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "The question, what you would do, and why." }
            },
            "required": ["question"]
        }
    });
    Description {
        instructions: vec![Instruction {
            observes: Observation {
                resource: ResourceKey {
                    limb,
                    kind: "instructions".into(),
                    name: String::new(),
                },
                version: Version::Is(Digest::of(said.as_bytes())),
            },
            text: said,
        }],
        tools: vec![Tool {
            name: ASK.into(),
            definition: ask.to_string(),
            audience: Audience::Agent,
        }],
    }
}
