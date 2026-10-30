//! Three replicas on the data flow substrate: the brain, Max's PC tab and his
//! phone, linked in memory. Run it with `demos/substrate/run.sh`.
//!
//! The PC and the phone never talk to each other, and nothing they send is
//! answered: each ships its own records to the brain, and the brain ships
//! each a view. Every line below is read off a replica.

use std::collections::BTreeMap;

use harness_core::build::{BuildId, Launch, Launched};
use harness_core::delivery::Read;
use harness_core::draft::{DraftEvent, Fate};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{DeliveryId, ReplicaId, RoleId, TaskId, Time, Timestamp, ValueId};
use harness_core::live::Live;
use harness_core::role::Role;
use harness_core::schema::{Brain, Harness};
use harness_core::state::State;
use harness_core::task::{
    Abandoner, Created, Drain, Life, Mandate, Mode, Origin, Outcome, Scope, TaskEvent,
};
use harness_core::value::Bytes;
use harness_substrate::{Down, Log, Memory, Mirror, Publisher, Status, View, What};

const BRAIN: ReplicaId = ReplicaId(0xb4a1_0000_0000_0001);
const PC: ReplicaId = ReplicaId(0x9c00_0000_0000_0002);
const PHONE: ReplicaId = ReplicaId(0xf0e0_0000_0000_0003);

/// Which row of a face's view.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Task(TaskId),
    Draft(DeliveryId),
}

/// One row of what a face shows, derived by the brain from its state.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Task {
        name: String,
        life: String,
    },
    Draft {
        /// The newest words, sent or not.
        text: Option<ValueId>,
        /// The words sent, and the face that sent them.
        sent: Option<(ValueId, ReplicaId)>,
        held: Option<String>,
        from: Option<DeliveryId>,
    },
}

impl Row {
    fn describe(&self, words: &BTreeMap<ValueId, String>) -> String {
        let quote = |id: &ValueId| format!("{:?}", words[id]);
        match self {
            Row::Task { name, life } => format!("session {name:?}, {life}"),
            Row::Draft {
                text,
                sent,
                held,
                from,
            } => {
                let mut out = match sent {
                    Some((sent, by)) => {
                        format!("draft, sent {} from the {}", quote(sent), name(*by))
                    }
                    None => format!("draft {}", text.as_ref().map_or("(empty)".into(), quote)),
                };
                if let (Some((sent, _)), Some(text)) = (sent, text)
                    && sent != text
                {
                    out += &format!(", typed since: {}", quote(text));
                }
                if let Some(from) = from {
                    out += &format!(", opened by a Send of draft {}", show(from.0.at));
                }
                if let Some(why) = held {
                    out += &format!(", not delivered: {why}");
                }
                out
            }
        }
    }
}

type FaceView = View<Key, Row, Refusal>;

/// What every face sees: every task and every draft. A real face's rows
/// are what its `Focus` selects.
fn rows(state: &State) -> BTreeMap<Key, Row> {
    let tasks = state.tasks().map(|t| {
        let life = match &t.life {
            Life::Starting => "starting".to_string(),
            Life::Live(_) => "live".to_string(),
            Life::Done {
                outcome: Outcome::Abandoned { by, .. },
                ..
            } => format!("done: stopped by {by:?}"),
            Life::Done { .. } => "done".to_string(),
        };
        let row = Row::Task {
            name: t.name.clone(),
            life,
        };
        (Key::Task(t.id), row)
    });
    let drafts = state.drafts().filter(|d| !d.discarded).map(|d| {
        let row = Row::Draft {
            text: d.text,
            sent: d.sent.map(|s| (s.text, s.by.at.replica)),
            held: d.held.as_ref().map(|why| why.to_string()),
            from: d.from,
        };
        (Key::Draft(d.id), row)
    });
    tasks.chain(drafts).collect()
}

/// A tab: its own log, and its replica of the brain's view.
struct Face {
    name: &'static str,
    log: Log<Harness, Memory<Harness>>,
    mirror: Mirror<Key, Row, Refusal>,
}

impl Face {
    fn new(name: &'static str, replica: ReplicaId) -> Face {
        Face {
            name,
            log: Log::open(replica, Memory::default()).unwrap(),
            mirror: Mirror::default(),
        }
    }

    fn view(&self) -> &FaceView {
        self.mirror.view().expect("a view arrives on connecting")
    }

    /// Max acts: one record, carrying what his screen showed.
    fn act(&mut self, clock: Timestamp, act: impl FnOnce(Time) -> Batch) -> Time {
        let seen = self.mirror.view().map(|v| v.as_of);
        let record = self
            .log
            .say(clock, |at| {
                let mut batch = act(at);
                batch.facts.extend(seen.map(Fact::Seen));
                batch
            })
            .unwrap();
        record.at
    }

    fn status(&self, at: Time) -> String {
        match self.view().standing.status(at.n) {
            Status::Pending => "pending".into(),
            Status::Included => "included".into(),
            Status::Refused(refusal) => format!("refused in band ({})", refusal.why),
        }
    }
}

/// The brain and the link state it keeps for each face: what it last
/// shipped, which is nothing it stores.
struct Hub {
    brain: Brain<Memory<Harness>>,
    publishers: BTreeMap<ReplicaId, Publisher<Key, Row, Refusal>>,
    clock: i64,
}

impl Hub {
    fn tick(&mut self) -> Timestamp {
        self.clock += 1_000;
        Timestamp(self.clock)
    }

    fn say(&mut self, said: impl FnOnce(Time) -> Batch) -> Time {
        let clock = self.tick();
        self.brain.say(clock, said).unwrap()
    }

    /// The face connects or is connected: it ships what the brain has not
    /// included, and the brain ships it what changed in its view.
    fn sync(&mut self, face: &mut Face) {
        let replica = face.log.here();
        for record in face.log.unsent(&self.brain.standing(replica)).unwrap() {
            let clock = self.tick();
            self.brain.include(clock, record).unwrap();
        }
        let view = View {
            as_of: self.brain.head(),
            standing: self.brain.standing(replica),
            rows: rows(self.brain.state()),
        };
        let publisher = self.publishers.entry(replica).or_default();
        if let Some(down) = publisher.publish::<Live>(view) {
            face.mirror.apply(down);
        }
        face.log.cover(&face.view().standing.clone()).unwrap();
    }

    fn live(&self, face: &mut Face, live: Live) {
        let down: Down<Key, Row, Refusal, Live> = Down::Live(harness_substrate::Live {
            after: self.brain.head(),
            live,
        });
        if let Down::Live(l) = &down {
            println!("  {} shows live, never stored: {:?}", face.name, l.live);
        }
        face.mirror.apply(down);
    }
}

fn name(replica: ReplicaId) -> &'static str {
    match replica {
        BRAIN => "brain",
        PC => "pc",
        PHONE => "phone",
        _ => "?",
    }
}

fn show(time: Time) -> String {
    format!("{}.{}", name(time.replica), time.n)
}

fn words(text: &str) -> (ValueId, Bytes) {
    let bytes = Bytes::from(text);
    (bytes.id(), bytes)
}

/// The bytes of `text`, remembered as a face's value cache would.
fn remember(cache: &mut BTreeMap<ValueId, String>, text: &str) -> (ValueId, Bytes) {
    let (id, bytes) = words(text);
    cache.insert(id, text.to_string());
    (id, bytes)
}

fn main() {
    let mut words_of: BTreeMap<ValueId, String> = BTreeMap::new();

    let (brain, _) = Brain::open(BRAIN, Memory::default()).unwrap();
    let mut hub = Hub {
        brain,
        publishers: BTreeMap::new(),
        clock: 1_792_000_000_000,
    };
    let mut pc = Face::new("pc", PC);
    let mut phone = Face::new("phone", PHONE);

    println!("1. The brain starts, accepts two faces, and defines a role.");
    let (role, role_bytes) = remember(&mut words_of, "You design.");
    let (params, params_bytes) = remember(&mut words_of, "{}");
    hub.say(|_| Batch {
        values: vec![],
        facts: vec![
            Fact::Launched(Launched {
                build: BuildId::of(b"demo"),
                source: None,
                limbs: Default::default(),
                how: Launch::Started,
            }),
            Fact::Face(PC),
            Fact::Face(PHONE),
        ],
    });
    let designer = hub.say(|at| Batch {
        values: vec![role_bytes, params_bytes],
        facts: vec![Fact::Role(Role {
            id: RoleId(at.event(0)),
            name: "designer".into(),
            prompt: role,
            model: "claude-opus-5-5".into(),
            params,
            tools: vec![],
            starts: vec![],
            source: None,
            description: String::new(),
        })],
    });
    let designer = RoleId(designer.event(0));
    hub.sync(&mut pc);
    hub.sync(&mut phone);

    println!("2. Max creates a session on the PC. Its id is the event that made it.");
    let (mandate, mandate_bytes) = remember(&mut words_of, "Design the substrate.");
    let clock = hub.tick();
    let created = pc.act(clock, |_| Batch {
        values: vec![mandate_bytes],
        facts: vec![Fact::Task(
            TaskId(PC.origin().event(0)),
            TaskEvent::Created(Box::new(Created {
                tuning: Default::default(),
                name: "substrate".into(),
                by: None,
                scope: Scope::Max,
                origin: Origin::Fresh {
                    role: designer,
                    workdir: None,
                    places: BTreeMap::new(),
                },
                mode: Mode::UserFacing,
                mandate: Mandate {
                    text: mandate,
                    brief: None,
                    quotes: vec![],
                },
            })),
        )],
    });
    println!(
        "  pc wrote {}; before syncing, the PC's own record says: {}",
        show(created),
        pc.status(created)
    );
    hub.sync(&mut pc);
    println!(
        "  a task id the PC did not mint at its own event is refused: {}",
        pc.status(created)
    );
    let clock = hub.tick();
    let created = pc.act(clock, |at| Batch {
        values: vec![words("Design the substrate.").1],
        facts: vec![Fact::Task(
            TaskId(at.event(0)),
            TaskEvent::Created(Box::new(Created {
                tuning: Default::default(),
                name: "substrate".into(),
                by: None,
                scope: Scope::Max,
                origin: Origin::Fresh {
                    role: designer,
                    workdir: None,
                    places: BTreeMap::new(),
                },
                mode: Mode::UserFacing,
                mandate: Mandate {
                    text: mandate,
                    brief: None,
                    quotes: vec![],
                },
            })),
        )],
    });
    let task = TaskId(created.event(0));
    hub.sync(&mut pc);
    hub.sync(&mut phone);
    println!(
        "  minted at its own event {}: {} at {}",
        show(created),
        pc.status(created),
        show(pc.view().as_of)
    );
    assert!(matches!(
        phone.view().rows.get(&Key::Task(task)),
        Some(Row::Task { .. })
    ));
    println!(
        "  the phone holds the session too: {}",
        phone.view().rows[&Key::Task(task)].describe(&words_of)
    );

    println!("3. The PC opens a draft and types offline. Only the newest text waits to ship.");
    let clock = hub.tick();
    let opened = pc.act(clock, |at| Batch {
        values: vec![],
        facts: vec![Fact::Draft(
            DeliveryId(at.event(0)),
            DraftEvent::Opened {
                session: task,
                at: None,
            },
        )],
    });
    let draft = DeliveryId(opened.event(0));
    let mut pc_text = ValueId::of(b"");
    for text in ["Let's", "Let's start", "Let's start with the data flow."] {
        let (id, bytes) = remember(&mut words_of, text);
        pc_text = id;
        let clock = hub.tick();
        pc.act(clock, |_| Batch {
            values: vec![bytes],
            facts: vec![Fact::Draft(draft, DraftEvent::Text(id))],
        });
    }
    println!(
        "  the PC's log holds {} records: the open, the newest text (the two before it were collected at the PC), and its last included one",
        pc.log.records().count()
    );
    hub.sync(&mut pc);
    hub.sync(&mut phone);
    let Some(Row::Draft { text, .. }) = phone.view().rows.get(&Key::Draft(draft)) else {
        panic!("the phone holds the draft");
    };
    println!(
        "  the phone shows the PC's draft: {:?}",
        words_of[&text.unwrap()]
    );

    println!("4. Both press Send on the same draft at once; the phone had added words.");
    let (phone_text, phone_bytes) =
        remember(&mut words_of, "Let's start with the data flow, and drafts.");
    let clock = hub.tick();
    let pc_send = pc.act(clock, |_| Batch {
        values: vec![words(&words_of[&pc_text]).1],
        facts: vec![Fact::Draft(
            draft,
            DraftEvent::Send {
                text: pc_text,
                read: Read::Next,
            },
        )],
    });
    let clock = hub.tick();
    phone.act(clock, |_| Batch {
        values: vec![phone_bytes.clone()],
        facts: vec![Fact::Draft(draft, DraftEvent::Text(phone_text))],
    });
    let clock = hub.tick();
    let phone_send = phone.act(clock, |_| Batch {
        values: vec![phone_bytes],
        facts: vec![Fact::Draft(
            draft,
            DraftEvent::Send {
                text: phone_text,
                read: Read::Next,
            },
        )],
    });
    println!(
        "  before syncing: pc's Send {}, phone's Send {}",
        pc.status(pc_send),
        phone.status(phone_send)
    );
    hub.sync(&mut pc);
    hub.sync(&mut phone);
    hub.sync(&mut pc);
    let state = hub.brain.state();
    let pc_fate = state.fate(draft, pc_send.event(0), pc_text);
    let phone_fate = state.fate(draft, phone_send.event(0), phone_text);
    println!(
        "  pc's Send: {}, {:?}; phone's Send: {}, {:?}",
        pc.status(pc_send),
        pc_fate,
        phone.status(phone_send),
        phone_fate
    );
    assert_eq!(pc_fate, Some(Fate::Delivered));
    assert_eq!(phone_fate, Some(Fate::Successor));
    let successor = DeliveryId(phone_send.event(0));
    for (face, other) in [(&pc, &phone), (&phone, &pc)] {
        assert_eq!(face.view().rows, other.view().rows);
        println!("  the {} shows:", face.name);
        for key in [Key::Draft(draft), Key::Draft(successor)] {
            println!("    {}", face.view().rows[&key].describe(&words_of));
        }
    }
    let delivered = hub.brain.state().delivery(draft).unwrap();
    let (harness_core::fact::Author::Max, Some(seen)) = (delivered.by, delivered.seen) else {
        panic!("Max sent it");
    };
    let face = hub
        .brain
        .state()
        .draft(draft)
        .unwrap()
        .sent
        .unwrap()
        .by
        .at
        .replica;
    println!(
        "  the delivery has the draft's id {}, by Max at the {}, who had seen the brain's view as of {}",
        show(draft.0.at),
        name(face),
        show(seen)
    );

    println!("5. Both press Stop at once.");
    let stop = |_: Time| Batch {
        values: vec![],
        facts: vec![Fact::Task(
            task,
            TaskEvent::Drain(Drain::Abandon {
                by: Abandoner::Max,
                within: 30_000,
            }),
        )],
    };
    let clock = hub.tick();
    let pc_stop = pc.act(clock, stop);
    let clock = hub.tick();
    let phone_stop = phone.act(clock, stop);
    hub.sync(&mut pc);
    hub.sync(&mut phone);
    println!(
        "  pc's Stop: {}; phone's Stop: {}",
        pc.status(pc_stop),
        phone.status(phone_stop)
    );
    assert!(matches!(
        phone.view().standing.status(phone_stop.n),
        Status::Refused(_)
    ));
    let held = hub.brain.storage().records().any(|r| r.at == phone_stop);
    println!("  the refused Stop is still held by the brain, as a fact: {held}");

    println!("6. The phone sends its new draft to the finished session. Its words are kept.");
    let clock = hub.tick();
    let late = phone.act(clock, |_| Batch {
        values: vec![words(&words_of[&phone_text]).1],
        facts: vec![Fact::Draft(
            successor,
            DraftEvent::Send {
                text: phone_text,
                read: Read::Next,
            },
        )],
    });
    hub.sync(&mut phone);
    println!(
        "  the Send: {}, {:?}; the phone shows: {}",
        phone.status(late),
        hub.brain.state().fate(successor, late.event(0), phone_text),
        phone.view().rows[&Key::Draft(successor)].describe(&words_of)
    );

    println!("7. Max clicks between sessions a hundred times.");
    let before = hub.brain.storage().records().count();
    for _ in 0..100 {
        let clock = hub.tick();
        pc.act(clock, |_| Batch {
            values: vec![],
            facts: vec![Fact::Focus(harness_core::attention::Focus {
                task: Some(task),
            })],
        });
        hub.sync(&mut pc);
    }
    let after = hub.brain.storage().records().count();
    println!("  the brain held {before} records before and {after} after");
    assert!(after <= before + 2);

    println!("8. Streamed tokens ride the same link and are never stored.");
    hub.live(
        &mut phone,
        Live::Model {
            call: harness_core::id::ModelCallId(BRAIN.origin().event(0)),
            chunk: "Thinking about".into(),
        },
    );

    println!("9. The brain crashes and restarts from what it stored.");
    let stored = hub.brain.storage().clone();
    let (brain, divergences) = Brain::open(BRAIN, stored).unwrap();
    assert!(divergences.is_empty());
    let replayed = brain.state() == hub.brain.state();
    println!("  replayed state equals live state: {replayed}");
    assert!(replayed);
    hub.brain = brain;
    hub.publishers.clear();
    let resent = pc.log.records().last().unwrap().clone();
    let again = hub.brain.include(Timestamp(hub.clock), resent).unwrap();
    println!("  the PC resends its last record anyway: the brain skips it ({again:?})");
    assert!(again.is_none());
    hub.sync(&mut pc);
    println!(
        "  the PC reconnects and gets a whole view as of {}",
        show(pc.view().as_of)
    );

    println!("10. The phone's storage is wiped, and it opens with the same replica id.");
    let restored: Log<Harness, Memory<Harness>> = Log::open(PHONE, Memory::default()).unwrap();
    match restored.unsent(&hub.brain.standing(PHONE)) {
        Err(restored) => println!(
            "  the brain holds the phone up to {}, the phone's storage only to {}: it continues as a new replica",
            restored.upto,
            restored.next - 1
        ),
        Ok(_) => panic!("a restored replica must not reuse its times"),
    }

    let kept = hub
        .brain
        .storage()
        .records()
        .filter(|r| matches!(&r.what, What::Said(b) if b.facts.iter().any(|f| matches!(f, Fact::Draft(_, DraftEvent::Text(_))))))
        .count();
    println!(
        "The brain keeps {kept} draft text records of the four typed: only each draft's newest."
    );
}
