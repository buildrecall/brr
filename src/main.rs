use anyhow::Result;
use clap::{AppSettings, Clap};
use notify::{RecursiveMode, Watcher};
use std::{path::Path, time::Duration};

mod global_config;
mod login;

/// This is a tool that makes your builds faster.
#[derive(Clap, Debug)]
#[clap(setting = AppSettings::ColoredHelp)]
#[clap(version = "0.1", author = "Build Recall")]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap, Debug)]
enum SubCommand {
    #[clap(version = "0.1", author = "Build Recall")]
    Login(login::Login),

    #[clap(version = "0.1", author = "Build Recall")]
    Attach(Attach),

    #[clap(version = "0.1", author = "Build Recall")]
    Detach(Detach),

    #[clap(version = "0.1", author = "Build Recall")]
    Logs(Logs),
}

/// Streams the build logs from the build farm
#[derive(Clap, Debug)]
struct Logs {}

/// Prebuilds this folder on the build farm
#[derive(Clap, Debug)]
struct Attach {}

/// Stops watching this folder on the build farm
#[derive(Clap, Debug)]
struct Detach {}

/// Login to
/// buildrecall login <token>
#[derive(Clap, Debug)]
struct Watch {
    /// A single-use, bash-history-safe token that
    /// confirms your login.
    token: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    match opts.subcmd {
        SubCommand::Login(t) => login::run_login(t).await,
        SubCommand::Attach(_) => Ok(()),
        SubCommand::Detach(_) => Ok(()),
        SubCommand::Logs(_) => Ok(()),
    }

    // watch_dir()?;
    // println!("watching..."); //1234

    // std::thread::sleep(Duration::from_secs(1000));
    // Ok(())
}

fn watch_dir() -> Result<()> {
    // Automatically select the best implementation for your platform.
    let mut watcher = notify::recommended_watcher(|res| match res {
        Ok(event) => println!("event: {:?}", event),
        Err(e) => println!("watch error: {:?}", e),
    })?;

    //

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(Path::new("."), RecursiveMode::Recursive)?; //
    println!("watching {:?}", std::env::current_dir()); //1234qawedo
    std::thread::sleep(Duration::from_secs(1000));

    Ok(())
}

fn send_latest_commit() {}

fn send_diff() {}
