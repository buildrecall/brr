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

        // TOML doesn't like empty arrays
        if existing.len() == 0 {
            return GlobalConfig {
                repos: None,
                connection: c.connection,
            };
        }

        GlobalConfig {
            repos: Some(existing),
            connection: c.connection,
        }
    })?;

    Ok(())
}
