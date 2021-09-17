use std::env;

use anyhow::{Context, Result};
use attach::AttachArguments;
use clap::{AppSettings, Clap};
#[cfg(target_os = "macos")]
use daemon::create_macos_launch_agent;
use global_config::get_global_config_dir;

use crate::hash::list_non_ignored_files_in_dir;

mod api;
mod attach;
mod daemon;
mod detatch;
mod git;
mod global_config;
mod hash;
mod invite;
mod login;
mod pull;
mod push;
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
    Invite(Invite),

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
    Hash(Empty),

    #[clap()]
    #[doc(hidden)]
    Daemon(Empty),

    #[clap()]
    #[doc(hidden)]
    Push(Empty),
}

/// Creates an invite link you can give to your team
#[derive(Clap, Debug)]
struct Invite {}

/// Streams the build logs from the build farm
#[derive(Clap, Debug)]
struct Logs {}

/// Prebuilds this folder on the build farm
#[derive(Clap, Debug)]
struct Attach {
    /// A name for this project that other folks on your team can understand
    pub name: Option<String>,
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

#[derive(Clap, Debug)]
struct Push {}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    match opts.subcmd {
        SubCommand::Login(l) => login::run_login(get_global_config_dir()?, l).await,
        SubCommand::Attach(args) => {
            worker_client::init().context("Failed to start worker client.")?;
            #[cfg(target_os = "macos")]
            create_macos_launch_agent()
                .context("Failed to start the Build Recall syncing daemon.")?;

            attach::run_attach(
                get_global_config_dir()?,
                AttachArguments { slug: args.name },
            )
            .await
        }
        SubCommand::Detach(_) => detatch::run_detach(get_global_config_dir()?).await,
        SubCommand::Logs(_) => todo!(),
        SubCommand::Pull(_) => pull::run_pull(get_global_config_dir()?).await,
        SubCommand::Daemon(_) => daemon::summon_daemon(get_global_config_dir()?).await,
        SubCommand::Hash(_) => {
            let curr = env::current_dir()?.as_path().to_path_buf();
            let files = list_non_ignored_files_in_dir(&curr.clone())
                .context("failed to list files in current dir")?;
            let hash = hash::hash_files(&curr.clone(), files)
                .await
                .context("failed to hash files")?;
            println!("{}", hash);
            Ok(())
        }
        SubCommand::Invite(_) => invite::run_invite(get_global_config_dir()?).await,
        SubCommand::Push(_) => push::run_push(get_global_config_dir()?).await,
    }
}
