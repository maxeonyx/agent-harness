//! The brain's side of its limbs' links: the substrate's [`LimbUp`] and
//! [`LimbDown`], as values over a channel for a limb on a thread of the
//! brain's process, and as JSON lines over the pipes of the `attach` child
//! that reaches a limb running on its own ([`crate::process`]).
//!
//! A limb says [`Hello`] first: which limb it is, the replica its records
//! are of, and its build. The brain ships it a whole view on every link, so
//! a limb outlives the brain that started it and serves the next.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc::{self, Sender};
use std::thread;

use harness_core::environment::{Hello, LimbDown, LimbUp};

/// One connection, numbered by the brain so a stale one's end is told
/// from the current one's.
pub type Conn = u64;

/// How the brain ships one limb what it holds of the brain: a channel to
/// the limb's thread, or to the thread writing the limb's link. So a
/// slow or stuck limb never holds up the brain; what it has not read
/// waits in the channel. A link that has gone is noticed by its reader.
pub type Wire = Sender<LimbDown>;

/// What a link says to the brain.
pub enum Said {
    Joined(Hello, Wire),
    Up(Box<LimbUp>),
    Left,
}

/// Follow one link: its hello, what it ships, until it closes. What the
/// brain ships it goes to `writer` from a thread of its own.
pub fn follow(
    conn: Conn,
    reader: impl Read,
    mut writer: impl Write + Send + 'static,
    said: &impl Fn(Conn, Said),
) -> io::Result<()> {
    let mut lines = BufReader::new(reader).lines();
    let Some(first) = lines.next() else {
        return Ok(());
    };
    let hello: Hello = serde_json::from_str(&first?).map_err(io::Error::other)?;
    let (wire, down) = mpsc::channel::<LimbDown>();
    thread::spawn(move || {
        for down in down {
            let mut line = serde_json::to_vec(&down).expect("a link value serialises");
            line.push(b'\n');
            if writer
                .write_all(&line)
                .and_then(|()| writer.flush())
                .is_err()
            {
                return;
            }
        }
    });
    said(conn, Said::Joined(hello, wire));
    for line in lines {
        let line = line?;
        match serde_json::from_str(&line) {
            Ok(up) => said(conn, Said::Up(Box::new(up))),
            Err(e) => errln!(
                "limb {} said something unreadable ({e}): {line}",
                hello.limb
            ),
        }
    }
    Ok(())
}
