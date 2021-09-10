use anyhow::Result;
use std::{env, path::PathBuf};

use crate::global_config::{overwrite_global_config, GlobalConfig, RepoConfig};

pub async fn run_detach(global_config_dir: PathBuf) -> Result<()> {
    let path = env::current_dir()?;

    overwrite_global_config(global_config_dir, move |c| {
        let empty = vec![];
        let repos = c.clone().repos.unwrap_or(empty);
        let existing = repos
            .iter()
            .filter(|r| r.path != path)
            .cloned()
            .collect::<Vec<RepoConfig>>();

        GlobalConfig {
            connection: c.connection,
            repos: Some(existing),
        }
    })?;

    Ok(())
}
