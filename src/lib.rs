#[cfg(target_os = "macos")]
use daemon::create_macos_launch_agent;

pub use crate::{config_global::get_global_config_dir, hash::list_non_ignored_files_in_dir};

pub mod api;
pub mod attach;
pub mod config_global;
pub mod config_local;
pub mod daemon;
pub mod detatch;
pub mod git;
pub mod hash;
pub mod invite;
pub mod login;
pub mod push;
pub mod run;
pub mod secrets;
