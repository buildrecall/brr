use anyhow::{anyhow, Context, Result};
use clap::Clap;
use serde::{Deserialize, Serialize};

use crate::global_config::{overwrite_global_config, GlobalConfig};

/// Login to
/// buildrecall login <token>
#[derive(Clap, Debug)]
pub struct Login {
    /// A single-use, bash-history-safe token that
    /// confirms your login.
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequestBody {
    pub single_use_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginRequestResponseBody {
    // The "real" access token that's saved on the user's
    // computer and used to auth
    pub access_token: String,
}

async fn post_cli_login(token: String) -> Result<String> {
    let client = reqwest::Client::new();

    let resp = client
        .post("http://localhost:8080/v1/cli/login")
        .json(&LoginRequestBody {
            single_use_token: token.clone(),
        })
        .send()
        .await
        .context("Failed to connect to Build Recall. Perhaps your internet is down, or Build Recall is having an outage.")?;

    if resp.status() == 401 {
        return Err(anyhow!(
            "The token '{}' is expired, invalid, or has already been used to login.\nYou can get a new one at https://buildrecall.com/setup",
            token
        ));
    }
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to login to Build Recall. Got a status code '{}'. Build Recall may be having an outage, or you may need to try running this command again.", resp.status()));
    }
    let result = resp.json::<LoginRequestResponseBody>()
        .await
        .context("Failed to login to Build Recall. The response unexpectedly did not return a JSON body. This is almost certainly a bug in Build Recall.")?;
    let token = result.access_token;

    Ok(token)
}

pub async fn run_login(login: Login) -> Result<()> {
    let tok = post_cli_login(login.token).await?;

    overwrite_global_config(GlobalConfig {
        access_token: Some(tok),
    })?;

    Ok(())
}
