use std::{
    fs,
    path::{self, Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};

use crate::global_config::read_global_config;

fn copy_folder<U: AsRef<Path>, V: AsRef<Path>>(from: U, to: V) -> Result<()> {
    let mut stack = Vec::new();
    stack.push(PathBuf::from(from.as_ref()));

    let output_root = PathBuf::from(to.as_ref());
    let input_root = PathBuf::from(from.as_ref()).components().count();

    while let Some(working_path) = stack.pop() {
        let src: PathBuf = working_path.components().skip(input_root).collect();

        // Create a destination if missing
        let dest = if src.components().count() == 0 {
            output_root.clone()
        } else {
            output_root.join(&src)
        };
        if fs::metadata(&dest).is_err() {
            fs::create_dir_all(&dest)?;
        }

        for entry in fs::read_dir(working_path.clone())
            .context(format!("Failed to read dir {:?}", working_path))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                match path.file_name() {
                    Some(filename) => {
                        let dest_path = dest.join(filename);
                        fs::copy(&path, &dest_path).context(format!(
                            "Failed to copy file from {:?} to {:?}",
                            path, dest_path
                        ))?;
                    }
                    None => return Err(anyhow!("failed to read file_name: {:?}", path)),
                }
            }
        }
    }

    Ok(())
}

pub fn worktree_path(global_config_dir: PathBuf, project_id: uuid::Uuid) -> Result<PathBuf> {
    let config = read_global_config(global_config_dir.clone())?;
    let repoconfig = config
        .repo_config_by_id(project_id)
        .ok_or(anyhow!("No project in config with id: {}", project_id))?;

    Ok(path::Path::new(&repoconfig.path).to_path_buf())
}

pub fn repo_path(global_config_dir: PathBuf, project_id: uuid::Uuid) -> Result<PathBuf> {
    Ok(global_config_dir.join(".gits").join(project_id.to_string()))
}

pub fn create_shadow_git_folder(global_config_dir: PathBuf, project_id: uuid::Uuid) -> Result<()> {
    // Create the .git
    let new_path = repo_path(global_config_dir.clone(), project_id)?;
    let repo = git2::Repository::init_bare(new_path)?;

    // Populate the work tree
    // let tree = repo
    //     .worktree(
    //         "buildrecall",
    //         path::Path::new(&worktree_path(global_config_dir.clone(), project_id)?),
    //         None,
    //     )
    //     .context("Failed to create the git worktree")?;

    // fs::create_dir_all(new_path.clone())?;
    // copy_folder(, new_path)?;

    Ok(())
}
