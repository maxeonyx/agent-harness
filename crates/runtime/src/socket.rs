//! A TCP connection that a stop cuts at once.

use std::io::{self, Read, Write};
use std::mem;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use crate::cancel::{Cancel, Cancelled, Waiter, Wake};

/// A TCP connection owned by one piece of work. Its connect, and each read
/// and write, wait in `poll` beside the work's [`Cancel`], so a request to
/// stop ends any of them at once in [`Cancelled`], wherever the work is
/// blocked: connecting, in a TLS handshake, waiting for a first byte, or
/// mid-stream. A connection has no graceful phase of its own: its owner
/// decides whether to let it finish.
///
/// **Vision.** Cancellable while blocked, not only between events; Max's
/// Stop on a thinking agent ends its request in milliseconds.
pub struct Socket {
    stream: TcpStream,
    waiter: Waiter,
    /// How long a read or write may wait with nothing moving.
    idle: Duration,
}

impl Socket {
    /// Connect to `host`'s `port`, giving up after `limit`. Name resolution
    /// cannot be cancelled; everything after it can.
    pub fn connect(
        host: &str,
        port: u16,
        cancel: &Cancel,
        limit: Duration,
        idle: Duration,
    ) -> io::Result<Socket> {
        let mut waiter = cancel.waiter()?;
        if cancel.is_requested() {
            return Err(Cancelled::error());
        }
        let until = Instant::now() + limit;
        let mut last = io::Error::new(io::ErrorKind::NotFound, format!("{host} has no address"));
        for addr in (host, port).to_socket_addrs()? {
            match attempt(&addr, &mut waiter, until) {
                Ok(stream) => {
                    stream.set_nodelay(true)?;
                    return Ok(Socket {
                        stream,
                        waiter,
                        idle,
                    });
                }
                Err(e) if Cancelled::is(&e) || e.kind() == io::ErrorKind::TimedOut => {
                    return Err(e);
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Run a non-blocking operation until it is done, waiting between tries
    /// for the socket to be ready, the stop, or the idle limit.
    fn blocking<T>(
        &mut self,
        events: i16,
        mut op: impl FnMut(&mut TcpStream) -> io::Result<T>,
    ) -> io::Result<T> {
        let fd = self.stream.as_raw_fd();
        loop {
            if self.waiter.cancel().is_requested() {
                return Err(Cancelled::error());
            }
            match op(&mut self.stream) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                done => return done,
            }
            let until = Instant::now() + self.idle;
            if self.waiter.wait(&[(fd, events)], Some(until))? == Wake::TimedOut {
                let idle = self.idle.as_secs();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("nothing moved for {idle}s"),
                ));
            }
        }
    }
}

impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.blocking(libc::POLLIN, |s| s.read(buf))
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.blocking(libc::POLLOUT, |s| s.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Connect a non-blocking socket to `addr`, waiting for it beside the stop.
fn attempt(addr: &SocketAddr, waiter: &mut Waiter, until: Instant) -> io::Result<TcpStream> {
    let (storage, len) = sockaddr(addr);
    let family = match addr {
        SocketAddr::V4(_) => libc::AF_INET,
        SocketAddr::V6(_) => libc::AF_INET6,
    };
    let kind = libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
    // SAFETY: socket only makes a descriptor.
    let fd = unsafe { libc::socket(family, kind, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a new descriptor nothing else owns.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    // SAFETY: `storage` holds a sockaddr of `len` bytes.
    let r = unsafe { libc::connect(fd.as_raw_fd(), (&raw const storage).cast(), len) };
    if r < 0 {
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(e);
        }
        loop {
            if waiter.cancel().is_requested() {
                return Err(Cancelled::error());
            }
            match waiter.wait(&[(fd.as_raw_fd(), libc::POLLOUT)], Some(until))? {
                Wake::Ready => break,
                Wake::Changed => {}
                Wake::TimedOut => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("connecting to {addr} took too long"),
                    ));
                }
            }
        }
        let mut error: libc::c_int = 0;
        let mut size = mem::size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: SO_ERROR writes one c_int.
        let r = unsafe {
            libc::getsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                (&raw mut error).cast(),
                &mut size,
            )
        };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error));
        }
    }
    Ok(TcpStream::from(fd))
}

fn sockaddr(addr: &SocketAddr) -> (libc::sockaddr_storage, libc::socklen_t) {
    // SAFETY: all-zero is a valid sockaddr_storage.
    let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
    let len = match addr {
        SocketAddr::V4(a) => {
            let sin = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: a.port().to_be(),
                sin_addr: libc::in_addr {
                    s_addr: u32::from_ne_bytes(a.ip().octets()),
                },
                sin_zero: [0; 8],
            };
            // SAFETY: sockaddr_storage is larger than, and aligned for, sockaddr_in.
            unsafe { (&raw mut storage).cast::<libc::sockaddr_in>().write(sin) };
            mem::size_of::<libc::sockaddr_in>()
        }
        SocketAddr::V6(a) => {
            let sin6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: a.port().to_be(),
                sin6_flowinfo: a.flowinfo(),
                sin6_addr: libc::in6_addr {
                    s6_addr: a.ip().octets(),
                },
                sin6_scope_id: a.scope_id(),
            };
            // SAFETY: sockaddr_storage is larger than, and aligned for, sockaddr_in6.
            unsafe { (&raw mut storage).cast::<libc::sockaddr_in6>().write(sin6) };
            mem::size_of::<libc::sockaddr_in6>()
        }
    };
    (storage, len as libc::socklen_t)
}
