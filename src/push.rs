use anyhow::{anyhow, Context, Result};
use std::{env, path::PathBuf};

use crate::{config_local, git::RecallGit};

pub async fn run_push(global_config_dir: PathBuf) -> Result<()> {
    let config = config_local::read_local_config(env::current_dir()?)?;

    let name = config
        .project()
        .name
        .ok_or(anyhow!("This project doesn't have a name property"))?;

    let g = RecallGit::new(global_config_dir).context("Failed to create shadow git")?;
    g.push_project(name, false)
        .await
        .context("Failed to push to shadow git repo")?;

    Ok(())
}

pub async fn run_push_in_current_dir_retry(global_config_dir: PathBuf, slug: String) -> Result<()> {
    let g = RecallGit::new(global_config_dir).context("Failed to create shadow git")?;

    g.push_project(slug, true)
        .await
        .context("Failed to push to shadow git repo")?;

    Ok(())
}
