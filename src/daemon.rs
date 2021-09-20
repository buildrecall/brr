use anyhow::{anyhow, Context, Result};
use ignore::gitignore::Gitignore;
use notify::{watcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{self, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;
use tracing::error;

use crate::git::RecallGit;
use crate::global_config::read_global_config;

#[cfg(target_os = "macos")]
pub fn create_macos_launch_agent() -> Result<()> {
    use std::{fs::create_dir_all, process::Command};

    let bin = std::env::current_exe()?;
    let home = dirs::home_dir().ok_or(anyhow!(
        "Can't find a $HOME directory (aka ~), which is needed on MacOS to\n start the background process that syncs repos with the build farm."
    ))?;

    create_dir_all(home.join("Library").join("Logs").join("buildrecall"))?;

    let stdout_log = home
        .join("Library")
        .join("Logs")
        .join("buildrecall")
        .join("buildrecall.out.log");

    let stderr_log = home
        .join("Library")
        .join("Logs")
        .join("buildrecall")
        .join("buildrecall.err.log");

    let xml = format!(
        r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.buildrecall.daemon</string>

  <key>RunAtLoad</key>
  <true/>

  <key>KeepAlive</key>
  <true/>

  <key>StandardOutPath</key>
  <string>{}</string>
  <key>StandardErrorPath</key>
  <string>{}</string>

  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>daemon</string>
  </array>
</dict>
</plist>
    "#,
        stdout_log.to_str().unwrap(),
        stderr_log.to_str().unwrap(),
        bin.to_str().unwrap()
    );

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(
            path::Path::new(&home)
                .join("Library")
                .join("LaunchAgents")
                .join("com.buildrecall.daemon.plist"),
        )
        .context("Failed to open ~/Library/LaunchAgents/com.buildrecall.daemon.plist")?;

    file.write_all(xml.as_bytes())?;

    Command::new("launchctl")
        .args([
            "load",
            "-w",
            "~/Library/LaunchAgents/com.buildrecall.daemon.plist",
        ])
        .output()
        .expect("failed to start Buildrecall launch agent");

    Ok(())
}

pub async fn summon_daemon(global_config_dir: PathBuf) -> Result<()> {
    println!("Starting daemon");

    let g = RecallGit::new(global_config_dir.clone())?;
    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(10)).unwrap();

    let config = read_global_config(global_config_dir).context("Failed to read global config")?;
    if config.repos.is_none() {
        return Err(anyhow!(
            "You need to attach a repo before starting the Build Recall Daemon."
        ));
    }

    let mut ignores: HashMap<PathBuf, Gitignore> = HashMap::new();

    for repo in config.repos.clone().unwrap_or(vec![]) {
        let (gi, err) = Gitignore::new(repo.path.clone().join(".gitignore"));
        if err.is_some() {
            return Err(err.unwrap().into()); // fails if there's no git ignore
        }

        ignores.insert(repo.path.clone(), gi);
        watcher
            .watch(repo.path.clone(), RecursiveMode::Recursive)
            .unwrap();
    }

    loop {
        match rx.recv() {
            Ok(event) => {
                let path = match event {
                    notify::DebouncedEvent::NoticeWrite(p) => p,
                    notify::DebouncedEvent::NoticeRemove(p) => p,
                    notify::DebouncedEvent::Create(p) => p,
                    notify::DebouncedEvent::Write(p) => p,
                    notify::DebouncedEvent::Chmod(p) => p,
                    notify::DebouncedEvent::Remove(p) => p,
                    notify::DebouncedEvent::Rename(_, p) => p,
                    notify::DebouncedEvent::Rescan => continue,
                    notify::DebouncedEvent::Error(e, _) => {
                        tracing::error!("{:?}", e);
                        continue;
                    }
                };

                let maybe_repo = config.repo_config_of_pathbuf(path.clone())?;
                let repo = match maybe_repo {
                    Some(r) => r,
                    None => return Err(anyhow!("expected to find a repo {:?}", path)),
                };

                match g.push_project(repo.id).await {
                    Ok(_) => continue,
                    Err(e) => error!("Push failed: {}", e.to_string()),
                };
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }
}
