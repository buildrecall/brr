use anyhow::{anyhow, Result};
use std::path::PathBuf;

use crate::{config_global::read_global_config, git::RecallGit};

pub async fn run_push(global_config_dir: PathBuf) -> Result<()> {
    let config = read_global_config(global_config_dir.clone())?;
    let repoconfig = config
        .clone()
        .repo_config_of_current_dir()?
        .ok_or(anyhow!(
            "This isn't a buildrecall project, try running attach first."
        ))?
        .clone();

    let g = RecallGit::new(global_config_dir)?;
    g.push_project(repoconfig.id).await?;

    Ok(())
}
