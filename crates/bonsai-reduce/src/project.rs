//! Multi-file project management with command-pattern undo support.
//!
//! [`ProjectFileSet`] loads a directory of source files into a temp directory,
//! classifies them as roots or dependencies, and supports tentative modifications
//! (modify, exclude) that can be rolled back when the interestingness test rejects
//! a change.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Directories to skip when loading a project.
const SKIP_DIRS: &[&str] = &[".git", ".hg", "target", "node_modules", "__pycache__"];

/// A reversible file-system command.
pub trait FileCommand {
    /// Apply the command to the temp directory.
    fn apply(&self, temp_root: &Path) -> io::Result<()>;
    /// Undo the command in the temp directory.
    fn undo(&self, temp_root: &Path) -> io::Result<()>;
}

/// Overwrite a file with new contents; undo restores old contents.
pub struct ModifyCommand {
    pub relative_path: PathBuf,
    pub new_contents: Vec<u8>,
    pub old_contents: Vec<u8>,
}

impl FileCommand for ModifyCommand {
    fn apply(&self, temp_root: &Path) -> io::Result<()> {
        fs::write(temp_root.join(&self.relative_path), &self.new_contents)
    }

    fn undo(&self, temp_root: &Path) -> io::Result<()> {
        fs::write(temp_root.join(&self.relative_path), &self.old_contents)
    }
}

/// Remove a file from the temp directory; undo re-creates it.
pub struct ExcludeCommand {
    pub relative_path: PathBuf,
    pub contents: Vec<u8>,
}

impl FileCommand for ExcludeCommand {
    fn apply(&self, temp_root: &Path) -> io::Result<()> {
        let path = temp_root.join(&self.relative_path);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn undo(&self, temp_root: &Path) -> io::Result<()> {
        let path = temp_root.join(&self.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &self.contents)
    }
}

/// Manages a working set of project files in a temp directory.
///
/// Files are classified as *roots* (the files being reduced) or *dependencies*
/// (context files needed for the interestingness test). The command pattern
/// enables rollback when tentative changes fail.
pub struct ProjectFileSet {
    temp_dir: TempDir,
    files: HashMap<PathBuf, Vec<u8>>,
    roots: HashSet<PathBuf>,
    commands: HashMap<PathBuf, Vec<Box<dyn FileCommand>>>,
}

impl ProjectFileSet {
    /// Load all files from `dir`, copying them into a temporary directory.
    ///
    /// `roots` lists relative paths that are considered root files (the reduction
    /// targets). Returns an error if any declared root is not found in the directory.
    ///
    /// Symlinks are skipped with a warning on stderr. Directories named `.git`,
    /// `.hg`, `target`, `node_modules`, or `__pycache__` are skipped entirely.
    pub fn from_directory(dir: &Path, roots: &[PathBuf]) -> io::Result<Self> {
        let temp_dir = TempDir::new()?;
        let mut files = HashMap::new();
        let root_set: HashSet<PathBuf> = roots.iter().cloned().collect();

        Self::load_recursive(dir, dir, temp_dir.path(), &mut files)?;

        // Verify all declared roots exist
        for root in &root_set {
            if !files.contains_key(root) {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("declared root not found: {}", root.display()),
                ));
            }
        }

        Ok(Self {
            temp_dir,
            files,
            roots: root_set,
            commands: HashMap::new(),
        })
    }

    fn load_recursive(
        base: &Path,
        current: &Path,
        temp_root: &Path,
        files: &mut HashMap<PathBuf, Vec<u8>>,
    ) -> io::Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;

            // Skip symlinks (use file_type() which doesn't follow symlinks)
            if file_type.is_symlink() {
                eprintln!(
                    "warning: skipping symlink: {}",
                    path.display()
                );
                continue;
            }

            let relative = path.strip_prefix(base).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e.to_string())
            })?;

            if file_type.is_dir() {
                let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
                if SKIP_DIRS.contains(&dir_name.as_ref()) {
                    continue;
                }
                // Create corresponding dir in temp
                fs::create_dir_all(temp_root.join(relative))?;
                Self::load_recursive(base, &path, temp_root, files)?;
            } else if file_type.is_file() {
                let contents = fs::read(&path)?;
                let temp_path = temp_root.join(relative);
                if let Some(parent) = temp_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&temp_path, &contents)?;
                files.insert(relative.to_path_buf(), contents);
            }
        }
        Ok(())
    }

    /// Overwrite a file with new contents, recording the change for undo.
    pub fn update_file(&mut self, path: &Path, new_contents: Vec<u8>) -> io::Result<()> {
        let old_contents = self
            .files
            .get(path)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("file not in project: {}", path.display()),
                )
            })?
            .clone();

        let cmd = ModifyCommand {
            relative_path: path.to_path_buf(),
            new_contents: new_contents.clone(),
            old_contents,
        };
        cmd.apply(self.temp_dir.path())?;

        self.files.insert(path.to_path_buf(), new_contents);
        self.commands
            .entry(path.to_path_buf())
            .or_default()
            .push(Box::new(cmd));
        Ok(())
    }

    /// Remove a file from the project. Returns an error if the file is a root.
    pub fn exclude_file(&mut self, path: &Path) -> io::Result<()> {
        if self.roots.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cannot exclude root file: {}", path.display()),
            ));
        }

        let contents = self
            .files
            .remove(path)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("file not in project: {}", path.display()),
                )
            })?;

        let cmd = ExcludeCommand {
            relative_path: path.to_path_buf(),
            contents,
        };
        cmd.apply(self.temp_dir.path())?;

        self.commands
            .entry(path.to_path_buf())
            .or_default()
            .push(Box::new(cmd));
        Ok(())
    }

    /// Undo the last command for the given file path.
    /// Re-synchronizes in-memory state from the temp directory.
    pub fn undo_last(&mut self, path: &Path) -> io::Result<()> {
        let cmds = self.commands.get_mut(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no commands to undo for: {}", path.display()),
            )
        })?;

        let cmd = cmds.pop().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no commands to undo for: {}", path.display()),
            )
        })?;

        cmd.undo(self.temp_dir.path())?;

        // Re-sync in-memory state from temp dir
        let temp_path = self.temp_dir.path().join(path);
        if temp_path.exists() {
            let contents = fs::read(&temp_path)?;
            self.files.insert(path.to_path_buf(), contents);
        } else {
            self.files.remove(path);
        }

        // Clean up empty command vecs
        if cmds.is_empty() {
            self.commands.remove(path);
        }

        Ok(())
    }

    /// Return all root file paths.
    pub fn root_files(&self) -> Vec<PathBuf> {
        self.roots.iter().cloned().collect()
    }

    /// Return all dependency (non-root) file paths, sorted by size descending.
    pub fn dependency_files(&self) -> Vec<PathBuf> {
        let mut deps: Vec<_> = self
            .files
            .iter()
            .filter(|(p, _)| !self.roots.contains(*p))
            .map(|(p, c)| (p.clone(), c.len()))
            .collect();
        deps.sort_by(|a, b| b.1.cmp(&a.1));
        deps.into_iter().map(|(p, _)| p).collect()
    }

    /// Path to the temporary directory containing the working copies.
    pub fn temp_dir_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Get a file's contents by relative path.
    pub fn get_file(&self, path: &Path) -> Option<&[u8]> {
        self.files.get(path).map(|v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a test directory with some files and return (dir, root_path, dep_path).
    fn setup_test_dir() -> (TempDir, PathBuf, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("main.py");
        let dep = dir.path().join("lib.py");
        let sub = dir.path().join("pkg");
        fs::create_dir(&sub).unwrap();
        let sub_file = sub.join("util.py");

        fs::write(&root, b"print('hello')").unwrap();
        fs::write(&dep, b"def helper(): pass").unwrap();
        fs::write(&sub_file, b"# util").unwrap();

        // Create a .git dir that should be skipped
        let git = dir.path().join(".git");
        fs::create_dir(&git).unwrap();
        fs::write(git.join("HEAD"), b"ref: refs/heads/main").unwrap();

        (dir, PathBuf::from("main.py"), PathBuf::from("lib.py"))
    }

    #[test]
    fn test_from_directory_loads_files() {
        let (dir, root, _dep) = setup_test_dir();
        let pfs = ProjectFileSet::from_directory(dir.path(), &[root.clone()]).unwrap();

        assert_eq!(pfs.get_file(Path::new("main.py")), Some(b"print('hello')" as &[u8]));
        assert_eq!(pfs.get_file(Path::new("lib.py")), Some(b"def helper(): pass" as &[u8]));
        assert_eq!(
            pfs.get_file(Path::new("pkg/util.py")),
            Some(b"# util" as &[u8])
        );
        // .git should be skipped
        assert_eq!(pfs.get_file(Path::new(".git/HEAD")), None);
    }

    #[test]
    fn test_from_directory_copies_to_temp() {
        let (dir, root, _) = setup_test_dir();
        let pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let temp_main = pfs.temp_dir_path().join("main.py");
        assert!(temp_main.exists());
        assert_eq!(fs::read(&temp_main).unwrap(), b"print('hello')");
    }

    #[test]
    fn test_from_directory_root_not_found() {
        let (dir, _, _) = setup_test_dir();
        let result = ProjectFileSet::from_directory(
            dir.path(),
            &[PathBuf::from("nonexistent.py")],
        );
        assert!(result.is_err());
        let err = result.err().expect("should be an error");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(err.to_string().contains("nonexistent.py"));
    }

    #[test]
    fn test_root_and_dependency_classification() {
        let (dir, root, _dep) = setup_test_dir();
        let pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let roots = pfs.root_files();
        assert_eq!(roots.len(), 1);
        assert!(roots.contains(&PathBuf::from("main.py")));

        let deps = pfs.dependency_files();
        assert!(!deps.contains(&PathBuf::from("main.py")));
        assert!(deps.contains(&PathBuf::from("lib.py")));
        assert!(deps.contains(&PathBuf::from("pkg/util.py")));
    }

    #[test]
    fn test_dependency_files_sorted_by_size_desc() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("root.py");
        fs::write(&root, b"x").unwrap();

        // Create files of different sizes
        fs::write(dir.path().join("small.py"), b"ab").unwrap(); // 2 bytes
        fs::write(dir.path().join("medium.py"), b"abcdef").unwrap(); // 6 bytes
        fs::write(dir.path().join("large.py"), b"abcdefghij").unwrap(); // 10 bytes

        let pfs = ProjectFileSet::from_directory(
            dir.path(),
            &[PathBuf::from("root.py")],
        )
        .unwrap();

        let deps = pfs.dependency_files();
        // Should be sorted largest first
        let sizes: Vec<usize> = deps
            .iter()
            .map(|p| pfs.get_file(p).unwrap().len())
            .collect();
        for w in sizes.windows(2) {
            assert!(w[0] >= w[1], "deps not sorted by size desc: {:?}", sizes);
        }
    }

    #[test]
    fn test_modify_and_undo_round_trip() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let path = Path::new("main.py");
        let original = pfs.get_file(path).unwrap().to_vec();

        // Modify
        pfs.update_file(path, b"print('modified')".to_vec()).unwrap();
        assert_eq!(pfs.get_file(path), Some(b"print('modified')" as &[u8]));
        assert_eq!(
            fs::read(pfs.temp_dir_path().join(path)).unwrap(),
            b"print('modified')"
        );

        // Undo
        pfs.undo_last(path).unwrap();
        assert_eq!(pfs.get_file(path).unwrap(), original.as_slice());
        assert_eq!(
            fs::read(pfs.temp_dir_path().join(path)).unwrap(),
            original
        );
    }

    #[test]
    fn test_exclude_and_undo_round_trip() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let path = Path::new("lib.py");
        let original = pfs.get_file(path).unwrap().to_vec();

        // Exclude
        pfs.exclude_file(path).unwrap();
        assert_eq!(pfs.get_file(path), None);
        assert!(!pfs.temp_dir_path().join(path).exists());

        // Undo
        pfs.undo_last(path).unwrap();
        assert_eq!(pfs.get_file(path).unwrap(), original.as_slice());
        assert!(pfs.temp_dir_path().join(path).exists());
    }

    #[test]
    fn test_exclude_root_fails() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let result = pfs.exclude_file(Path::new("main.py"));
        assert!(result.is_err());
        let err = result.err().expect("should be an error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("cannot exclude root"));
    }

    #[test]
    fn test_multi_step_undo() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let path = Path::new("main.py");
        let v0 = pfs.get_file(path).unwrap().to_vec();

        pfs.update_file(path, b"v1".to_vec()).unwrap();
        pfs.update_file(path, b"v2".to_vec()).unwrap();
        pfs.update_file(path, b"v3".to_vec()).unwrap();

        assert_eq!(pfs.get_file(path), Some(b"v3" as &[u8]));

        pfs.undo_last(path).unwrap();
        assert_eq!(pfs.get_file(path), Some(b"v2" as &[u8]));

        pfs.undo_last(path).unwrap();
        assert_eq!(pfs.get_file(path), Some(b"v1" as &[u8]));

        pfs.undo_last(path).unwrap();
        assert_eq!(pfs.get_file(path).unwrap(), v0.as_slice());

        // No more commands to undo
        assert!(pfs.undo_last(path).is_err());
    }

    #[test]
    fn test_update_nonexistent_file() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let result = pfs.update_file(Path::new("nope.py"), b"data".to_vec());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_exclude_nonexistent_file() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let result = pfs.exclude_file(Path::new("nope.py"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_skips_target_and_node_modules() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.rs"), b"fn main() {}").unwrap();

        for skip_dir in &["target", "node_modules", "__pycache__"] {
            let d = dir.path().join(skip_dir);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("junk.txt"), b"skip me").unwrap();
        }

        let pfs = ProjectFileSet::from_directory(
            dir.path(),
            &[PathBuf::from("main.rs")],
        )
        .unwrap();

        assert_eq!(pfs.get_file(Path::new("target/junk.txt")), None);
        assert_eq!(pfs.get_file(Path::new("node_modules/junk.txt")), None);
        assert_eq!(pfs.get_file(Path::new("__pycache__/junk.txt")), None);
    }

    #[test]
    fn test_exclude_then_dependency_files_updates() {
        let (dir, root, _) = setup_test_dir();
        let mut pfs = ProjectFileSet::from_directory(dir.path(), &[root]).unwrap();

        let deps_before = pfs.dependency_files();
        assert!(deps_before.contains(&PathBuf::from("lib.py")));

        pfs.exclude_file(Path::new("lib.py")).unwrap();

        let deps_after = pfs.dependency_files();
        assert!(!deps_after.contains(&PathBuf::from("lib.py")));
    }
}
