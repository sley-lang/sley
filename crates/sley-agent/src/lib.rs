//! Sley 2 agent workbench (`sley-agent`).
//!
//! A machine-efficient interface over the kernel libraries for the agents
//! that author Sley programs (`docs/spec/SLEY_AGENT_V1.md`, ADR-0051):
//!
//! - AV1 views: compact, output-only, non-canonical listings under local
//!   names ([`view`]), focused views ([`focus`]) and the AF1-X-shaped
//!   AV1-X rendering ([`xview`]).
//! - AF1 frames: name-based JSON authoring data compiled client-side, in one
//!   pass, into the existing mutation candidate record ([`frame`]), and the
//!   AF1-X authoring dialect expanded into them first ([`afx`]), with its
//!   `ripple` graph transformations ([`ripple`]).
//! - An advisory dev loop: in-process candidate validation with decoded
//!   refusals and locators, and lower-once execution of functions and
//!   `TestCases` ([`exec`]).
//! - Verified search: bounded, typed local repair proposals checked by the
//!   kernel and the author's public cases, never submitted ([`search`]).
//!
//! - The optional familiar frontend (cargo feature `familiar`, ADR-0055): a
//!   textual proposal syntax parsed into structured bodies at the authoring
//!   boundary, then handled like any JSON frame ([`familiar`]).
//! - The embed API (with `familiar`): compile a familiar program in memory
//!   through the `try --familiar` path and call its functions under an
//!   exact fuel limit, with every input of nondeterminism supplied by the
//!   host ([`embed`], `docs/EMBEDDING.md`).
//!
//! The cargo feature `native` (default) adds the file-system layer:
//! workspaces, repositories, drafts, search and the CLI. Without it the
//! library is pure and builds for `wasm32-unknown-unknown`.
//!
//! The kernel alone judges candidates. Nothing here is admission evidence,
//! and nothing here commits or signs. The familiar frontend is the one
//! parser of program text; it is optional, its text is never canonical, and
//! no other path depends on it.

// The library has no unsafe code. The binary's allocator is the one exception
// in the workspace, and it is a module of the binary target (ADR-0052).
#![forbid(unsafe_code)]
// Without `native`, the helpers the frame compiler shares with the
// native-only residual engine, search and CLI have no caller; they stay
// compiled so that both builds share one implementation.
#![cfg_attr(not(feature = "native"), allow(dead_code))]

pub mod afx;
pub mod candidate;
pub mod catalog;
#[cfg(feature = "native")]
pub mod cli;
#[cfg(feature = "native")]
pub mod draft;
#[cfg(feature = "familiar")]
pub mod embed;
pub mod error;
#[cfg(feature = "native")]
pub mod events;
pub mod exec;
pub mod explain;
#[cfg(feature = "familiar")]
pub mod familiar;
pub mod focus;
pub mod frame;
pub mod genesis;
pub mod help;
pub mod hex;
pub mod layer;
pub mod locate;
pub mod names;
pub mod opcodes;
pub mod raw;
#[cfg(feature = "native")]
pub mod residual;
pub mod ripple;
#[cfg(feature = "native")]
pub mod search;
pub(crate) mod structured;
pub mod tables;
pub mod types;
pub mod values;
pub mod view;
pub mod workspace;
pub mod xview;

pub use error::{AgentError, AgentErrorCode, Result};
