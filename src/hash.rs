use anyhow::{anyhow, Context, Result};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use ignore::gitignore::Gitignore;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn list_non_ignored_files_in_dir(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let (gi, err) = Gitignore::new(dir.clone().join(".gitignore"));

    // We can't tell wether or not there's a git ignore
    if err.is_some() {
        return Err(err.unwrap().into());
    }

    let paths = WalkDir::new(dir);

    let mut matches = vec![];
    for f in paths {
        let fres = f?;
        let fpath = fres.path();
        let fmeta = fres.metadata().context(format!(
            "Can't read the metadata of the file {:?}",
            fpath.clone()
        ))?;

        let stripped = fpath.strip_prefix(dir)?;
        let is_ignored = gi
            .matched_path_or_any_parents(stripped, fmeta.is_dir())
            .is_ignore();
        let is_git = stripped.starts_with(".git");
        if !is_ignored && !is_git {
            matches.push(fpath.to_path_buf());
        }
    }

    Ok(matches)
}

/// Computes a sha-3 hash of the files in sorted order
/// hash = sha3(bytes(file_path_relative_to_root) + bytes(file_contents))
pub async fn hash_files(root: &PathBuf, paths: Vec<PathBuf>) -> Result<String> {
    let mut sorted = paths.clone();
    sorted.sort_by(|a, b| b.cmp(a));

    // Start hash
    let mut hasher = Sha3::sha3_256();

    for path in sorted {
        if path.is_dir() {
            continue;
        }

        /*
        Potentially could be a bug here: Executables on windows vs unix.
        - On Windows, every file is executable
        - On Unix, only files where metadata.mode() & 0o111 != 0

        If I make a file executable in git on unix, it will store that, so when I git pull, any file someone else marked as exec is now exec.

        This might lead to problems where, someone creates a file on a windows computer, that's supposed to be executable, but the unix build farm server doesn't know it's supposed to be exec
        */

        let cloned_path = path.clone();
        let result = cloned_path
            .strip_prefix(root)
            .context(format!("{:?} is not a child of {:?}", path, root))?;
        let as_str = result
            .to_str()
            .ok_or(anyhow!("Failed to convert {:?} to string", result))?;

        let mut filepath = as_str.as_bytes().to_vec();
        let mut contents = fs::read(path.clone())
            .context(format!("Failed to read this file: {:?}", path.clone()))?;

        let mut input = vec![];
        input.append(&mut filepath);
        input.append(&mut contents);

        hasher.input(&input);
    }

    Ok(hasher.result_str())
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use itertools::Itertools;
    use std::fs::{create_dir_all, File};

    use std::io::Write;
    use std::path::Path;
    use std::vec;
    use tempdir::TempDir;

    use crate::hash::hash_files;

    use super::list_non_ignored_files_in_dir;

    // Checks that subdirectories are being included in hashes
    #[tokio::test]
    async fn test_recursive_differences() {
        // Create a folder with some code.
        let tmp_1 = TempDir::new(".recursive_dir_1")
            .context("Can't create a tmp dir")
            .unwrap();
        create_dir_all(tmp_1.path().join("folder")).unwrap();
        let a = tmp_1.path().join("alpha.flargle");
        let mut afile = File::create(a.clone()).unwrap();
        afile.write_all(b"Some *() text here; () => {}").unwrap();
        let b = tmp_1.path().join("folder").join("231asb21.json");
        let mut bfile = File::create(b.clone()).unwrap();
        bfile.write_all(b"One thing here; () => {}").unwrap();

        // Compute the original hash
        let root_1 = &tmp_1.path().to_path_buf();
        let files_1 = list_non_ignored_files_in_dir(&root_1.clone()).unwrap();
        let hash_1 = hash_files(&root_1.clone(), files_1).await.unwrap();

        // Change the code in a subdirectory
        bfile.write_all(b"a totally different thing").unwrap();

        // Compute the new hash
        let root_2 = &tmp_1.path().to_path_buf();
        let files_2 = list_non_ignored_files_in_dir(&root_2.clone()).unwrap();
        let hash_2 = hash_files(&root_2.clone(), files_2).await.unwrap();

        // Show they're different
        assert_ne!(hash_1, hash_2);
    }

    // checks that two repos produce the same hash
    #[tokio::test]
    async fn test_equivalence() {
        // Create a folder with some code code.
        let tmp_1 = TempDir::new(".recursive_dir_1")
            .context("Can't create a tmp dir")
            .unwrap();
        create_dir_all(tmp_1.path().join("folder")).unwrap();
        let a = tmp_1.path().join("alpha.flargle");
        let mut afile = File::create(a.clone()).unwrap();
        afile.write_all(b"Some *() text here; () => {}").unwrap();
        let b = tmp_1.path().join("folder").join("231asb21.json");
        let mut bfile = File::create(b.clone()).unwrap();
        bfile.write_all(b"One thing here; () => {}").unwrap();

        // Create a different folder with the same code
        let tmp_2 = TempDir::new(".recursive_dir_2")
            .context("Can't create a tmp dir")
            .unwrap();
        create_dir_all(tmp_2.path().join("folder")).unwrap();
        let a = tmp_2.path().join("alpha.flargle");
        let mut afile = File::create(a.clone()).unwrap();
        afile.write_all(b"Some *() text here; () => {}").unwrap();
        let b = tmp_2.path().join("folder").join("231asb21.json");
        let mut bfile = File::create(b.clone()).unwrap();
        bfile.write_all(b"One thing here; () => {}").unwrap();

        // Hash both files, outputs should be the asme
        let root_1 = &tmp_1.path().to_path_buf();
        let files_1 = list_non_ignored_files_in_dir(&root_1.clone()).unwrap();
        let hash_1 = hash_files(&root_1.clone(), files_1).await.unwrap();
        let root_2 = tmp_2.path().to_path_buf();
        let files_2 = list_non_ignored_files_in_dir(&root_2.clone()).unwrap();
        let hash_2 = hash_files(&root_2.clone(), files_2).await.unwrap();

        assert_eq!(hash_1, hash_2);
    }

    // checks that the order of the hashed files doesn't change the outcome
    #[tokio::test]
    async fn test_with_different_permutations() {
        let tmp = TempDir::new(".hash")
            .context("Can't create a tmp dir")
            .unwrap();

        let a = tmp.path().join("alpha.flargle");
        let mut afile = File::create(a.clone()).unwrap();
        afile.write_all(b"Some *() text here; () => {}").unwrap();

        let b = tmp.path().join("231asb21.json");
        let mut bfile = File::create(b.clone()).unwrap();
        bfile.write_all(b"ablawefome text here; () => {}").unwrap();

        let c = tmp.path().join("zed.bargle");
        let mut cfile = File::create(c.clone()).unwrap();
        cfile.write_all(b"ac ahhhhh here; () => {}").unwrap();

        let d = tmp.path().join("derga.azzz");
        let mut afile = File::create(d.clone()).unwrap();
        afile.write_all(b"aaaa text here; () => {}").unwrap();

        let e = tmp.path().join("derga");
        let mut efile = File::create(e.clone()).unwrap();
        efile.write_all(b"Some text here; () => {}").unwrap();

        let f = tmp.path().join("smarshmellow");
        let mut ffile = File::create(f.clone()).unwrap();
        ffile.write_all(b"Some text here; () => {}").unwrap();

        let files = vec![a, b, c, d, e, f];

        let paths = files
            .iter()
            .permutations(files.len())
            .unique()
            .collect::<Vec<_>>();

        let mut hashes: Vec<String> = vec![];
        for p in paths {
            let hash = hash_files(
                &Path::new(tmp.as_ref()).to_path_buf(),
                p.clone().iter().cloned().map(|e| e.clone()).collect_vec(),
            )
            .await
            .unwrap();
            hashes.push(hash);
        }

        assert!(hashes.iter().all_equal());
    }
}
