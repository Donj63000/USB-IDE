use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::fs::{detect_text_encoding, is_probably_binary, read_text_with_encoding};

const INTERNAL_ROOT_DIRS: [&str; 6] = [".git", ".usbide", "cache", "codex_home", "target", "tmp"];

#[derive(Debug, Clone)]
pub struct WorkspacePaths {
    root_dir: PathBuf,
    cache_pip: PathBuf,
    cache_pycache: PathBuf,
    cache_npm: PathBuf,
    tmp_dir: PathBuf,
    codex_home: PathBuf,
    bug_log_path: PathBuf,
    usbide_dir: PathBuf,
    usbide_codex: PathBuf,
    usbide_tools: PathBuf,
    tools_node: PathBuf,
    tools_wheels: PathBuf,
    dist_dir: PathBuf,
}

impl WorkspacePaths {
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            cache_pip: root_dir.join("cache").join("pip"),
            cache_pycache: root_dir.join("cache").join("pycache"),
            cache_npm: root_dir.join("cache").join("npm"),
            tmp_dir: root_dir.join("tmp"),
            codex_home: root_dir.join("codex_home"),
            bug_log_path: root_dir.join("bug.md"),
            usbide_dir: root_dir.join(".usbide"),
            usbide_codex: root_dir.join(".usbide").join("codex"),
            usbide_tools: root_dir.join(".usbide").join("tools"),
            tools_node: root_dir.join("tools").join("node"),
            tools_wheels: root_dir.join("tools").join("wheels"),
            dist_dir: root_dir.join("dist"),
            root_dir,
        }
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn bug_log_path(&self) -> &Path {
        &self.bug_log_path
    }

    pub fn tools_node(&self) -> &Path {
        &self.tools_node
    }

    pub fn dist_dir(&self) -> &Path {
        &self.dist_dir
    }

    pub fn ensure_portable_dirs(&self) {
        for path in [
            &self.cache_pip,
            &self.cache_pycache,
            &self.cache_npm,
            &self.tmp_dir,
            &self.codex_home,
            &self.usbide_dir,
            &self.usbide_codex,
            &self.usbide_tools,
        ] {
            let _ = fs::create_dir_all(path);
        }
    }

    pub fn portable_env(
        &self,
        mut env_map: std::collections::HashMap<String, String>,
    ) -> std::collections::HashMap<String, String> {
        env_map.insert(
            "PIP_CACHE_DIR".to_string(),
            self.cache_pip.display().to_string(),
        );
        env_map.insert(
            "PYTHONPYCACHEPREFIX".to_string(),
            self.cache_pycache.display().to_string(),
        );
        env_map.insert("TEMP".to_string(), self.tmp_dir.display().to_string());
        env_map.insert("TMP".to_string(), self.tmp_dir.display().to_string());
        env_map.insert("PYTHONNOUSERSITE".to_string(), "1".to_string());
        env_map.insert(
            "CODEX_HOME".to_string(),
            self.codex_home.display().to_string(),
        );
        env_map.insert(
            "NPM_CONFIG_CACHE".to_string(),
            self.cache_npm.display().to_string(),
        );
        env_map.insert(
            "NPM_CONFIG_UPDATE_NOTIFIER".to_string(),
            "false".to_string(),
        );
        env_map
    }

    pub fn wheelhouse_path(&self) -> Option<PathBuf> {
        if self.tools_wheels.is_dir() {
            Some(self.tools_wheels.clone())
        } else {
            None
        }
    }

    pub fn is_internal_path(&self, path: &Path) -> bool {
        let relative = match self.relative_path(path) {
            Some(path) => path,
            None => return false,
        };

        relative
            .components()
            .find_map(|component| match component {
                std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
                _ => None,
            })
            .map(|name| INTERNAL_ROOT_DIRS.iter().any(|hidden| hidden == &name))
            .unwrap_or(false)
    }

    pub fn is_sensitive_path(&self, path: &Path) -> bool {
        let relative = match self.relative_path(path) {
            Some(path) => path,
            None => return false,
        };
        relative == Path::new("codex_home").join("auth.json")
    }

    pub fn should_display_in_tree(&self, path: &Path) -> bool {
        !self.is_internal_path(path)
    }

    fn relative_path(&self, path: &Path) -> Option<PathBuf> {
        if path.is_absolute() {
            path.strip_prefix(&self.root_dir)
                .ok()
                .map(Path::to_path_buf)
        } else {
            Some(path.to_path_buf())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
struct FileNode {
    path: PathBuf,
    name: String,
    is_dir: bool,
    children: Vec<FileNode>,
}

#[derive(Debug, Clone)]
pub struct FileTreeData {
    root: FileNode,
    expanded: HashSet<PathBuf>,
    visible: Vec<TreeEntry>,
}

impl FileTreeData {
    pub fn new(workspace: &WorkspacePaths) -> Self {
        let root = build_tree(workspace.root_dir(), workspace);
        let mut expanded = HashSet::new();
        expanded.insert(root.path.clone());
        let mut tree = Self {
            root,
            expanded,
            visible: Vec::new(),
        };
        tree.rebuild_visible();
        tree
    }

    pub fn reload(&mut self, workspace: &WorkspacePaths) {
        let root = build_tree(workspace.root_dir(), workspace);
        self.root = root;
        self.expanded.retain(|path| path.exists());
        self.expanded.insert(self.root.path.clone());
        self.rebuild_visible();
    }

    pub fn toggle_dir(&mut self, path: &Path) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
        }
        self.rebuild_visible();
    }

    pub fn visible(&self) -> &[TreeEntry] {
        &self.visible
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    fn rebuild_visible(&mut self) {
        self.visible.clear();
        flatten_tree(&self.root, 0, &self.expanded, &mut self.visible);
    }
}

#[derive(Debug, Clone)]
pub struct OpenedWorkspaceFile {
    pub path: PathBuf,
    pub encoding: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum OpenWorkspaceFileError {
    #[error("Fichier interne masque: {0}")]
    Hidden(PathBuf),
    #[error("Fichier sensible protege: {0}")]
    Sensitive(PathBuf),
    #[error("Binaire/non texte ignore: {0}")]
    Binary(PathBuf),
    #[error("Acces fichier impossible: {path} ({source})")]
    Access {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Erreur ouverture: {path} ({source})")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn open_workspace_file(
    workspace: &WorkspacePaths,
    path: PathBuf,
) -> Result<OpenedWorkspaceFile, OpenWorkspaceFileError> {
    if workspace.is_sensitive_path(&path) {
        return Err(OpenWorkspaceFileError::Sensitive(path));
    }
    if workspace.is_internal_path(&path) {
        return Err(OpenWorkspaceFileError::Hidden(path));
    }
    if path.is_dir() {
        return Err(OpenWorkspaceFileError::Hidden(path));
    }

    match is_probably_binary(&path, 2048) {
        Ok(true) => return Err(OpenWorkspaceFileError::Binary(path)),
        Ok(false) => {}
        Err(source) => {
            return Err(OpenWorkspaceFileError::Access { path, source });
        }
    }

    let encoding = detect_text_encoding(&path);
    let text = read_text_with_encoding(&path, &encoding).map_err(|source| {
        OpenWorkspaceFileError::Read {
            path: path.clone(),
            source,
        }
    })?;

    Ok(OpenedWorkspaceFile {
        path,
        encoding,
        text,
    })
}

fn build_tree(path: &Path, workspace: &WorkspacePaths) -> FileNode {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string());
    let is_dir = path.is_dir();
    let mut children = Vec::new();

    if is_dir && let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.flatten() {
            let child_path = entry.path();
            if !workspace.should_display_in_tree(&child_path) {
                continue;
            }
            children.push(build_tree(&child_path, workspace));
        }
        children.sort_by_key(|node| (!node.is_dir, node.name.to_lowercase()));
    }

    FileNode {
        path: path.to_path_buf(),
        name,
        is_dir,
        children,
    }
}

fn flatten_tree(
    node: &FileNode,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    out: &mut Vec<TreeEntry>,
) {
    out.push(TreeEntry {
        path: node.path.clone(),
        name: node.name.clone(),
        depth,
        is_dir: node.is_dir,
    });
    if node.is_dir && expanded.contains(&node.path) {
        for child in &node.children {
            flatten_tree(child, depth + 1, expanded, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn masque_dossiers_internes_dans_arborescence() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("cache").join("npm")).unwrap();
        fs::create_dir_all(dir.path().join("codex_home")).unwrap();
        fs::write(dir.path().join("src").join("main.py"), "print('ok')").unwrap();
        fs::write(dir.path().join("codex_home").join("auth.json"), "{}").unwrap();

        let workspace = WorkspacePaths::new(dir.path().to_path_buf());
        let tree = FileTreeData::new(&workspace);
        let names: Vec<String> = tree
            .visible()
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        assert!(names.iter().any(|name| name == "src"));
        assert!(!names.iter().any(|name| name == "cache"));
        assert!(!names.iter().any(|name| name == "codex_home"));
    }

    #[test]
    fn protege_auth_json() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("codex_home")).unwrap();
        let auth = dir.path().join("codex_home").join("auth.json");
        fs::write(&auth, "{\"token\":\"secret\"}").unwrap();

        let workspace = WorkspacePaths::new(dir.path().to_path_buf());
        let err = open_workspace_file(&workspace, auth).unwrap_err();

        assert!(matches!(err, OpenWorkspaceFileError::Sensitive(_)));
    }

    #[test]
    fn env_portable_pointe_vers_workspace() {
        let dir = TempDir::new().unwrap();
        let workspace = WorkspacePaths::new(dir.path().to_path_buf());
        let env = workspace.portable_env(std::collections::HashMap::new());

        assert_eq!(
            env.get("CODEX_HOME").unwrap(),
            &dir.path().join("codex_home").display().to_string()
        );
        assert_eq!(
            env.get("NPM_CONFIG_CACHE").unwrap(),
            &dir.path().join("cache").join("npm").display().to_string()
        );
    }

    #[test]
    fn cree_dossiers_portables() {
        let dir = TempDir::new().unwrap();
        let workspace = WorkspacePaths::new(dir.path().to_path_buf());
        workspace.ensure_portable_dirs();

        assert!(dir.path().join(".usbide").join("codex").is_dir());
        assert!(dir.path().join(".usbide").join("tools").is_dir());
        assert!(dir.path().join("cache").join("pip").is_dir());
        assert!(dir.path().join("codex_home").is_dir());
    }
}
