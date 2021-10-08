use std::{env, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use dialoguer::Confirm;

use crate::{
    api::{ApiClient, BuildRecall, Project},
    config_global::read_global_config,
    config_local::{overwrite_local_config, LocalConfig, ProjectConfig},
    git,
};

pub struct AttachArguments {
    pub slug: Option<String>,
}

pub async fn run_attach(global_config_dir: PathBuf, args: AttachArguments) -> Result<()> {
    let global_config = read_global_config(global_config_dir.clone()).context(format!(
        "Failed to read the global config file: {:?}",
        global_config_dir
    ))?;
    let client = ApiClient::new(global_config.clone());

    if global_config.clone().access_token().is_none() {
        return Err(anyhow!(
            "You're not logged in. You can login by going to https://buildrecall.com/setup"
        ));
    }

    let path = env::current_dir()?;
    let pieces = path
        .components()
        .map(|comp| comp.as_os_str().to_str().unwrap_or("").to_string())
        .collect::<Vec<_>>();
    let folder = pieces[pieces.len() - 1].clone();
    let slug = args.slug.unwrap_or(folder);

    let projects = client.list_projects().await?;

    // Check if there's already a project
    let maybe_project = projects.iter().find(|p| p.slug == slug.clone());
    let project: Result<Project> = match maybe_project {
        Some(p) => {
            if !Confirm::new()
                .with_prompt(format!(
                    "A project named '{}' already exists\nLink this project?",
                    slug.clone()
                ))
                .default(true)
                .interact()?
            {
                return Ok(());
            } else {
                Ok(p.clone())
            }
        }
        None => {
            let p = client
                .create_project(slug.clone())
                .await
                .context("Failed to create the project in Buildrecall")?
                .clone();
            Ok(p)
        }
    };

    // Create a local config file
    let local_slug = slug.clone();
    overwrite_local_config(
        env::current_dir().context("Failed to read current dir")?,
        move |c| LocalConfig {
            jobs: c.jobs,
            project: Some(ProjectConfig {
                name: Some(local_slug),
            }),
        },
    )
    .context("Failed to create buildrecall.toml")?;

    // create a .git folder for brr to use that doesn't mess with the user's git.
    let g = git::RecallGit::new(global_config_dir.clone())?;
    g.create_shadow_git_folder(slug)
        .context(format!("Failed to create a shadow git folder (used to sync files without messing with your own git setup) in {:?}/{}", global_config_dir, ".gits"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_adds_a() {
        assert_eq!(2 + 2, 4);
    }
}
