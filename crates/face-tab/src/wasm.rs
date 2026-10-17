//! The tab's exports to the page's JavaScript.
//!
//! The page is glue: the DOM, the socket and IndexedDB. Everything it
//! ships, stores or reads of the brain goes through here, parsed into
//! core's types on the way in.
//!
//! **The calling convention** is one buffer, with no bindings generator:
//! the page asks [`io`] for room, writes its input there (UTF-8 JSON, or a
//! value's bytes), calls an export, and reads the export's output from
//! [`io_ptr`] and [`io_len`]. An export that fails writes `{"error": why}`.
//! Several tabs may be open in one page (a closed tab's place, taken over
//! to ship what it had not), each by the handle [`open`] returned.

use std::cell::RefCell;

use harness_core::id::{Timestamp, ValueId};
use harness_substrate::{PlaceId, Status};
use serde_json::{Value, json};

use crate::{Down, Tab, Up};

thread_local! {
    static IO: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static TABS: RefCell<Vec<Option<Tab>>> = const { RefCell::new(Vec::new()) };
}

/// Room for `n` bytes of input.
#[unsafe(no_mangle)]
pub extern "C" fn io(n: usize) -> *mut u8 {
    IO.with_borrow_mut(|io| {
        io.clear();
        io.resize(n, 0);
        io.as_mut_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn io_ptr() -> *const u8 {
    IO.with_borrow(|io| io.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn io_len() -> usize {
    IO.with_borrow(|io| io.len())
}

fn input() -> Vec<u8> {
    IO.with_borrow(|io| io.clone())
}

fn output(value: Value) {
    IO.set(value.to_string().into_bytes());
}

fn error(why: impl std::fmt::Display) {
    output(json!({ "error": why.to_string() }));
}

fn with_tab(handle: u32, f: impl FnOnce(&mut Tab) -> Value) {
    let out = TABS.with_borrow_mut(|tabs| {
        tabs.get_mut(handle as usize)
            .and_then(Option::as_mut)
            .map(f)
    });
    match out {
        Some(value) => output(value),
        None => error("no such tab"),
    }
}

/// Open a tab from `{"place", "records"}`: its place id and what its
/// storage holds. Returns its handle, or `u32::MAX` with the error.
#[unsafe(no_mangle)]
pub extern "C" fn open() -> u32 {
    #[derive(serde::Deserialize)]
    struct Opening {
        place: PlaceId,
        records: Vec<harness_core::schema::Record>,
    }
    match serde_json::from_slice::<Opening>(&input()) {
        Ok(o) => TABS.with_borrow_mut(|tabs| {
            tabs.push(Some(Tab::open(o.place, o.records)));
            output(Value::Null);
            (tabs.len() - 1) as u32
        }),
        Err(e) => {
            error(e);
            u32::MAX
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn close(handle: u32) {
    TABS.with_borrow_mut(|tabs| {
        if let Some(tab) = tabs.get_mut(handle as usize) {
            *tab = None;
        }
    });
}

/// The time the next record will have, which names what it creates.
#[unsafe(no_mangle)]
pub extern "C" fn next(handle: u32) {
    with_tab(handle, |tab| json!(tab.next()));
}

/// Say the batch in the input, at `clock` (milliseconds since the epoch):
/// `{"n", "up"}`, the record's number and the message that ships it. Ship
/// it only once [`writes`] are durable.
#[unsafe(no_mangle)]
pub extern "C" fn say(handle: u32, clock: f64) {
    let batch = input();
    with_tab(handle, |tab| {
        let batch = String::from_utf8_lossy(&batch);
        match tab.say(Timestamp(clock as i64), &batch) {
            Ok(record) => json!({ "n": record.at.n, "up": Up::Record(record) }),
            Err(e) => json!({ "error": format!("not facts: {e}") }),
        }
    });
}

/// Take in a message from the brain: `{"live"}` if it was live progress
/// to show, else `{}` and the view has moved on.
#[unsafe(no_mangle)]
pub extern "C" fn down(handle: u32) {
    let text = input();
    with_tab(handle, |tab| match serde_json::from_slice::<Down>(&text) {
        Ok(down) => match tab.down(down) {
            Some(live) => json!({ "live": live }),
            None => json!({}),
        },
        Err(e) => json!({ "error": format!("not a view: {e}") }),
    });
}

/// What to ship on connecting: `{"ups"}`, or `{"restored"}` if the brain
/// holds more of this place than its storage does, and the tab must
/// continue as a new place.
#[unsafe(no_mangle)]
pub extern "C" fn unsent(handle: u32) {
    with_tab(handle, |tab| match tab.unsent() {
        Ok(records) => {
            let ups: Vec<Up> = records.into_iter().map(Up::Record).collect();
            json!({ "ups": ups })
        }
        Err(restored) => json!({ "restored": { "upto": restored.upto, "next": restored.next } }),
    });
}

/// The writes to make durable, in order: `[{"Put": record} | {"Drop": time}]`.
#[unsafe(no_mangle)]
pub extern "C" fn writes(handle: u32) {
    with_tab(handle, |tab| json!(tab.writes()));
}

/// The view held: `{"as_of", "standing", "rows"}`, or `null` before one
/// has arrived.
#[unsafe(no_mangle)]
pub extern "C" fn view(handle: u32) {
    with_tab(handle, |tab| json!(tab.view()));
}

/// Where record `n` stands: `"pending"`, `"included"`, or
/// `{"refused": why}`.
#[unsafe(no_mangle)]
pub extern "C" fn status(handle: u32, n: f64) {
    with_tab(handle, |tab| match tab.status(n as u64) {
        Status::Pending => json!("pending"),
        Status::Included => json!("included"),
        Status::Refused(why) => json!({ "refused": why.why.to_string() }),
    });
}

/// Whether everything the tab said is included.
#[unsafe(no_mangle)]
pub extern "C" fn settled(handle: u32) -> u32 {
    TABS.with_borrow(|tabs| {
        tabs.get(handle as usize)
            .and_then(Option::as_ref)
            .is_some_and(Tab::settled) as u32
    })
}

/// The id of the input's bytes, as the facts that name a value write it.
#[unsafe(no_mangle)]
pub extern "C" fn digest() {
    output(json!(ValueId::of(&input()).to_string()));
}
