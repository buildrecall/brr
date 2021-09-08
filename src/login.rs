use crate::api;
use anyhow::{anyhow, Context, Result};
use clap::Clap;
use std::path::PathBuf;

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
    pub token: String,
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
    let global_config = read_global_config(get_global_config_dir()?)?;

    let tok = post_cli_login(global_config, login.token).await?;

    let dir = get_global_config_dir()?;
    overwrite_global_config(dir, |c| GlobalConfig {
        access_token: Some(tok.clone()),
        repos: c.repos,
        host: c.host,
    })?;

    Ok(())
}
