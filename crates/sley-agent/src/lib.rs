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
//! The kernel alone judges candidates. Nothing here is admission evidence;
//! nothing here commits, signs, or parses Sley source.

// The library has no unsafe code. The binary's allocator is the one exception
// in the workspace, and it is a module of the binary target (ADR-0052).
#![forbid(unsafe_code)]

pub mod afx;
pub mod candidate;
pub mod catalog;
pub mod cli;
pub mod draft;
pub mod error;
pub mod events;
pub mod exec;
pub mod explain;
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
pub mod residual;
pub mod ripple;
pub mod search;
pub(crate) mod structured;
pub mod tables;
pub mod types;
pub mod values;
pub mod view;
pub mod workspace;
pub mod xview;

pub use error::{AgentError, AgentErrorCode, Result};
