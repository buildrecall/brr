use crate::{api, global_config::ConnectionConfig};
use anyhow::{anyhow, Context, Result};
use clap::Clap;
use std::{path::PathBuf, process::Command};

use crate::{
    api::{ApiClient, BuildRecall},
    global_config::{
        get_global_config_dir, overwrite_global_config, read_global_config, GlobalConfig,
    },
};

/// Login to
/// buildrecall login <token>
#[derive(Clap, Debug)]
pub struct Login {
    /// A single-use, bash-history-safe token that
    /// confirms your login.
    pub token: Option<String>,
}

async fn post_cli_login(global_config: GlobalConfig, single_use_token: String) -> Result<String> {
    let client = ApiClient::new(global_config);

    let result = client
        .login(api::LoginRequestBody { single_use_token })
        .await?;
    let token = result.access_token;

    Ok(token)
}

pub async fn run_login(global_config_dir: PathBuf, login: Login) -> Result<()> {
    let global_config = read_global_config(global_config_dir)?;

    match login.token {
        Some(token) => {
            let tok = post_cli_login(global_config, token).await?;

            let dir = get_global_config_dir()?;
            overwrite_global_config(dir, |c| GlobalConfig {
                connection: Some(ConnectionConfig {
                    access_token: Some(tok.clone()),
                    control_host: Some(c.control_host()),
                    scheduler_host: Some(c.scheduler_host()),
                }),
                repos: c.repos,
            })?;

            Ok(())
        }
        None => {
            eprintln!("Get a login code at https://buildrecall.com/login");

            if cfg!(target_os = "macos") {
                Command::new("open")
                    .args(["https://buildrecall.com/login"])
                    .output()?;
            }

            Ok(())
        }
    }
}
