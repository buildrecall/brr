use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::{
    api::{ApiClient, BuildRecall},
    config_global::read_global_config,
};

pub async fn run_invite(global_config_dir: PathBuf) -> Result<()> {
    let global_config = read_global_config(global_config_dir)?;
    let client = ApiClient::new(global_config);

    let result = client
        .invite()
        .await
        .context("Failed to create a new invite token")?;

    // Not a debug log, this is the output of this command
    println!("https://buildrecall.com/invite?token={}", result.token);

    Ok(())
}
