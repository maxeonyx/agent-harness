//! The brain's side of its limbs' links: the substrate's [`LimbUp`] and
//! [`LimbDown`], as values over a channel for a limb on a thread of the
//! brain's process, and as JSON lines over a Unix socket for a limb that
//! is a process of its own.
//!
//! A process limb connects to the socket and says [`Hello`]: which limb it
//! is and the place its records are of. It can connect again, to this
//! brain or a restarted one on the same socket, and is shipped a whole view
//! each time. So a limb outlives the brain that started it.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::thread;

use harness_core::environment::{Hello, LimbDown, LimbUp};

/// One connection, numbered by the brain so a stale one's end is told
/// from the current one's.
pub type Conn = u64;

/// How the brain ships one limb what it holds of the brain.
pub enum Wire {
    Socket(UnixStream),
    Channel(Sender<LimbDown>),
}

impl Wire {
    /// Ship one value. A link that has gone is noticed by its reader.
    pub fn ship(&mut self, down: LimbDown) {
        match self {
            Wire::Socket(stream) => {
                let mut line = serde_json::to_vec(&down).expect("a link value serialises");
                line.push(b'\n');
                let _ = stream.write_all(&line);
            }
            Wire::Channel(sender) => {
                let _ = sender.send(down);
            }
        }
    }
}

/// What a link says to the brain.
pub enum Said {
    Joined(Hello, Wire),
    Up(Box<LimbUp>),
    Left,
}

/// Listen on `path` for process limbs, replacing a socket an earlier
/// brain left. Each connection's hello, what it ships and its end go to
/// `said`, with the connection's number. Detached: it holds nothing.
pub fn listen(
    path: &Path,
    said: impl Fn(Conn, Said) + Send + Sync + Clone + 'static,
) -> io::Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    thread::spawn(move || {
        for (conn, stream) in (1..).zip(listener.incoming()) {
            let Ok(stream) = stream else { continue };
            let said = said.clone();
            thread::spawn(move || {
                if let Err(e) = follow(conn, stream, &said) {
                    eprintln!("a limb's link: {e}");
                }
                said(conn, Said::Left);
            });
        }
    });
    Ok(())
}

fn follow(conn: Conn, stream: UnixStream, said: &impl Fn(Conn, Said)) -> io::Result<()> {
    let writer = stream.try_clone()?;
    let mut lines = BufReader::new(stream).lines();
    let Some(first) = lines.next() else {
        return Ok(());
    };
    let hello: Hello = serde_json::from_str(&first?).map_err(io::Error::other)?;
    said(conn, Said::Joined(hello, Wire::Socket(writer)));
    for line in lines {
        let line = line?;
        match serde_json::from_str(&line) {
            Ok(up) => said(conn, Said::Up(Box::new(up))),
            Err(e) => eprintln!(
                "limb {} said something unreadable ({e}): {line}",
                hello.limb
            ),
        }
    }
    Ok(())
}
