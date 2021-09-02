use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::path::Path;
use std::time::Duration;

fn main() -> Result<()> {
    watch_dir()?;
    println!("watching..."); //1234

    std::thread::sleep(Duration::from_secs(1000));
    Ok(())
}

fn watch_dir() -> Result<()> {
    // Automatically select the best implementation for your platform.
    let mut watcher = notify::recommended_watcher(|res| match res {
        Ok(event) => println!("event: {:?}", event),
        Err(e) => println!("watch error: {:?}", e),
    })?;

    //

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(Path::new("."), RecursiveMode::Recursive)?;

    Ok(())
}
