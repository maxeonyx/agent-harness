//! A session's program: everything that decides and renders a session's
//! requests beyond what the core's fold fixes. The brain only calls it.
//!
//! - [`render`]: what a request renders beyond the results it must, and
//!   the wording of everything the harness says to a model.
//! - [`policy`]: retries and backoff, breakpoint placement, fan-out gating
//!   on first byte, and how long things get to stop.
//! - [`meta`], [`web`]: the limbs the brain implements itself,
//!   their tools and what their calls mean.
//!
//! **The boundary.** The crate does no I/O: no files, processes, sockets,
//! clocks or entropy, only functions of the state, values and times it is
//! given. It depends on nothing that does, so it builds for `wasm32`,
//! like the core it is written against. A session's program can then
//! become a module of its own (Max: "WASM modules later [...] we should
//! try to do the separation, but we can keep it Rust") by moving this
//! seam, not by finding it.
//!
//! What the provider requires of a request (its identity line) is the
//! provider's, and given to [`render::root`] by the caller.

pub mod meta;
pub mod policy;
pub mod render;
pub mod web;
