use anyhow::{anyhow, Context, Result};
use std::{env, path::PathBuf};

use crate::{git::RecallGit, run::JobArgs};

pub async fn run_push_in_current_dir_retry(
    global_config_dir: PathBuf,
    slug: String,
    job_args: JobArgs,
) -> Result<()> {
    let g = RecallGit::new(global_config_dir).context("Failed to create shadow git")?;

    g.push_project(slug, true, job_args)
        .await
        .context("Failed to push to shadow git repo")?;

    Ok(())
}
