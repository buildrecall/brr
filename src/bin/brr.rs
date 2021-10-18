use std::env;

use anyhow::{Context, Result};
use clap::{AppSettings, Clap};
use init::AttachArguments;

use brr::{run::JobArgs, *};

/// This is a tool that makes your builds faster.
#[derive(Clap, Debug)]
#[clap(setting = AppSettings::ColoredHelp)]
#[clap(version = "0.0.15", author = "Build Recall")]
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
    Init(Init),

    #[clap()]
    Run(Run),

    #[clap()]
    #[doc(hidden)]
    Hash(Empty),

    #[clap()]
    Secrets(Secrets),
}

/// Creates a secret
#[derive(Clap, Debug)]
struct Secrets {
    #[clap(subcommand)]
    subcmd: secrets::SecretsSubCommand,
}

/// Creates an invite link you can give to your team
#[derive(Clap, Debug)]
struct Invite {}

/// Creates a buildrecall.toml for this repo
#[derive(Clap, Debug)]
struct Init {
    /// A name for this project that other folks on your team can understand
    pub name: Option<String>,
}

/// Starts a job, or waits for an existing one with the
/// same file hash and then downloads any artifacts.
///
/// Use this in CI to deploy your build.
#[derive(Clap, Debug)]
struct Run {
    /// The name of the job
    job: String,
    container: String,
}

#[derive(Clap, Debug)]
struct Push {}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    // You can handle information about subcommands by requesting their matches by name
    // (as below), requesting just the name used, or both at the same time
    match opts.subcmd {
        SubCommand::Login(l) => login::run_login(get_global_config_dir()?, l).await,
        SubCommand::Init(args) => {
            init::run_attach(
                get_global_config_dir()?,
                AttachArguments { slug: args.name },
            )
            .await
        }
        SubCommand::Secrets(s) => {
            secrets::run_secrets(s.subcmd, get_global_config_dir()?, env::current_dir()?).await
        }
        SubCommand::Run(a) => {
            run::pull_with_push_if_needed(
                get_global_config_dir()?,
                env::current_dir()?,
                JobArgs {
                    job: a.job,
                    container: a.container,
                },
            )
            .await
        }
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
    }
}
