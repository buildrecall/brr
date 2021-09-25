use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use uuid::Uuid;

use crate::{config_global::read_global_config, git::RecallGit};

pub async fn run_push(global_config_dir: PathBuf) -> Result<()> {
    let config = read_global_config(global_config_dir.clone())
        .context("Failed to read global config file")?;
    let repoconfig = config
        .clone()
        .repo_config_of_current_dir()?
        .ok_or(anyhow!(
            "This isn't a buildrecall project, try running attach first."
        ))?
        .clone();

    let g = RecallGit::new(global_config_dir).context("Failed to create shadow git")?;
    g.push_project(repoconfig.id, false)
        .await
        .context("Failed to push to shadow git repo")?;

    Ok(())
}

pub async fn run_push_in_current_dir_retry(
    global_config_dir: PathBuf,
    project_id: Uuid,
) -> Result<()> {
    let g = RecallGit::new(global_config_dir).context("Failed to create shadow git")?;
    let tree = g
        .get_repo_by_project(project_id)?
        .index()?
        .write_tree()?
        .to_string();

    g.push_project(project_id, true)
        .await
        .context("Failed to push to shadow git repo")?;

    Ok(())
}
