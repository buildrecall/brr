use anyhow::Result;
use attach::AttachArguments;
use clap::{AppSettings, Clap};
use global_config::get_global_config_dir;
use notify::{RecursiveMode, Watcher};
use std::{path::Path, time::Duration};
use worker_client::push_to_worker;

mod api;
mod attach;
mod daemon;
mod global_config;
mod login;
mod worker_client;

/// This is a tool that makes your builds faster.
#[derive(Clap, Debug)]
#[clap(setting = AppSettings::ColoredHelp)]
#[clap(version = "0.1", author = "Build Recall")]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap, Debug)]
struct Empty {}

#[derive(Clap, Debug)]
enum SubCommand {
    #[clap()]
    Login(login::Login),

    #[clap()]
    Attach(Attach),

    #[clap()]
    Detach(Detach),

    #[clap()]
    Logs(Logs),

    #[clap()]
    Pull(Pull),

    #[clap()]
    #[doc(hidden)]
    TestPush(Empty),

    #[clap()]
    #[doc(hidden)]
    Daemon(Empty),
}

/// Streams the build logs from the build farm
#[derive(Clap, Debug)]
struct Logs {}

/// Prebuilds this folder on the build farm
#[derive(Clap, Debug)]
struct Attach {
    /// A name for this project that other folks on your team can understand
    pub name: String,
}

/// Stop prebuilding this folder on the build farm
#[derive(Clap, Debug)]
struct Detach {}

/// Downloads the build farm's version of this folder
/// when it is finished building.
///
/// Use this in CI to deploy your build.
#[derive(Clap, Debug)]
struct Pull {}

/// Login to
/// buildrecall login <token>
#[derive(Clap, Debug)]
struct Watch {
    /// A single-use, bash-history-safe token that
    /// confirms your login.
    pub token: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    worker_client::init();

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    match opts.subcmd {
        SubCommand::Login(l) => login::run_login(get_global_config_dir()?, l).await,
        SubCommand::Attach(args) => {
            attach::run_attach(
                get_global_config_dir()?,
                AttachArguments { slug: args.name },
            )
            .await
        }
        SubCommand::Detach(_) => Ok(()),
        SubCommand::Logs(_) => Ok(()),
        SubCommand::Pull(_) => Ok(()),
        SubCommand::TestPush(_) => push_to_worker().await,
        SubCommand::Daemon(_) => daemon::summon_daemon(get_global_config_dir()?),
    }
}
