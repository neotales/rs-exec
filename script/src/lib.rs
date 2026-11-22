//! Build [`neotales_exec::Command`] values for script runtimes.
//!
//! Every module has the same four constructors: [`bash::new`] for a bare runtime with arguments,
//! [`bash::file`] for a script path, [`bash::inline`] for source text, and [`bash::script`] for a
//! string that may be either. Each returns an owned command, so the caller sets the working
//! directory, the environment and a logger on it.
//!
//! The runtime is resolved through [`neotales_exec::find`] on first use. When it is not installed,
//! the command carries the bare name (`bash`, `node`, `go`) and the spawn reports the missing
//! program.
#![doc = include_str!("../README.md")]

pub mod bash;
pub mod bun;
pub mod cmd;
mod common;
pub mod dotnet;
pub mod golang;
pub mod node;
pub mod powershell;
pub mod pwsh;
pub mod sh;
