//! A message record, written into a stopped brain's database as the brain
//! would write it, for a brain that reads messages before any brain writes
//! them. See `demos/messages-read`.
//!
//! `tell DB TO WORDS [QUOTE]`: the newest `message` call in the database
//! tells `TO` (`owner`, or a task's name) `WORDS`, handing on the words of
//! Max's labelled `QUOTE`. The fold refuses it as it would the brain's own.

use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, process};

use harness_core::call::Caller;
use harness_core::delivery::{Content, Delivery};
use harness_core::fact::{Batch, Fact};
use harness_core::id::{DeliveryId, Timestamp};
use harness_core::message::{MESSAGE, Message};
use harness_core::schema;
use harness_core::value::Bytes;
use harness_store::Store;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let [db, to, words, rest @ ..] = args.as_slice() else {
        fail("usage: tell DB TO WORDS [QUOTE]");
    };
    let store = Store::open(db).unwrap_or_else(|e| fail(&format!("{db}: {e}")));
    let (mut brain, _) = schema::Brain::open(store.replica(), store)
        .unwrap_or_else(|e| fail(&format!("{db} folds: {e}")));
    let state = brain.state();
    let (call, from) = (state.calls())
        .filter(|(_, c)| c.call.tool == MESSAGE)
        .filter_map(|(id, c)| match c.call.caller {
            Caller::Agent { task, .. } => Some((id, task)),
            _ => None,
        })
        .max()
        .unwrap_or_else(|| fail("no agent has made a `message` call"));
    let to = match to.as_str() {
        "owner" => state.owner(from),
        name => state.tasks().find(|t| t.name == name).map(|t| t.id),
    }
    .unwrap_or_else(|| fail(&format!("no task is `{to}`")));
    let quotes = (rest.first().iter())
        .map(|label| {
            (state.sayings())
                .map(|(v, _, _)| v)
                .find(|v| format!("max-{}", &v.to_string()[..8]) == **label)
                .unwrap_or_else(|| fail(&format!("none of Max's words are labelled `{label}`")))
        })
        .collect();
    let text = Bytes::from(words.as_str());
    let message = Message {
        call,
        words: text.id(),
        quotes,
    };
    let clock = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let said = brain.say(Timestamp(clock.as_millis() as i64), |at| Batch {
        values: vec![text],
        facts: vec![Fact::Delivery(Delivery {
            id: DeliveryId(at.event(0)),
            to,
            content: Content::Message(message),
        })],
    });
    match said {
        Ok(at) => println!("told {to} from {from} by {}, at {at}", call.label()),
        Err(e) => fail(&format!("refused: {e:?}")),
    }
}

fn fail(why: &str) -> ! {
    eprintln!("tell: {why}");
    process::exit(1)
}
