use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;

use crate::codex::{
    self, CodexApprovalPolicy, CodexSandboxMode, codex_entrypoint_js, codex_install_prefix,
    node_executable, tools_env as build_tools_env,
};
use crate::process::ProcHandle;
use crate::workspace::WorkspacePaths;

pub const APP_NAME: &str = "ValDev Pro v1";
pub const LOG_LIMIT: usize = 2000;

#[derive(Debug, Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub encoding: String,
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTarget {
    Main,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    Shell,
    PythonRun,
    CodexExec,
    CodexCaps,
    CodexLogin,
    CodexStatus,
    CodexInstall,
    DevTools,
    PyInstallerInstall,
    PyInstallerBuild,
}

#[derive(Debug)]
pub struct RunningProcess {
    pub handle: ProcHandle,
    pub kind: ProcessKind,
    pub target: LogTarget,
    pub contexte: String,
}

#[derive(Debug)]
pub struct AppCore {
    workspace: WorkspacePaths,
    last_issue_fingerprint: Option<String>,
    pub running: Vec<RunningProcess>,
    pub codex_install_attempted: bool,
    pub pyinstaller_install_attempted: bool,
}

impl AppCore {
    pub fn new(root_dir: PathBuf) -> Self {
        let root_dir = root_dir.canonicalize().unwrap_or(root_dir);
        Self {
            workspace: WorkspacePaths::new(root_dir),
            last_issue_fingerprint: None,
            running: Vec::new(),
            codex_install_attempted: false,
            pyinstaller_install_attempted: false,
        }
    }

    pub fn workspace(&self) -> &WorkspacePaths {
        &self.workspace
    }

    pub fn ensure_portable_dirs(&self) {
        self.workspace.ensure_portable_dirs();
    }

    pub fn portable_env(&self, env_map: HashMap<String, String>) -> HashMap<String, String> {
        self.workspace.portable_env(env_map)
    }

    pub fn sanitize_codex_env(&self, env_map: &mut HashMap<String, String>) {
        let allow_api_key = truthy(std::env::var("USBIDE_CODEX_ALLOW_API_KEY").ok().as_ref());
        let allow_custom_base = truthy(
            std::env::var("USBIDE_CODEX_ALLOW_CUSTOM_BASE")
                .ok()
                .as_ref(),
        );

        if !allow_api_key {
            env_map.remove("OPENAI_API_KEY");
            env_map.remove("CODEX_API_KEY");
        }
        if !allow_custom_base {
            env_map.remove("OPENAI_BASE_URL");
            env_map.remove("OPENAI_API_BASE");
            env_map.remove("OPENAI_API_HOST");
        }
    }

    pub fn codex_env(&self) -> HashMap<String, String> {
        let mut env_map: HashMap<String, String> = std::env::vars().collect();
        env_map
            .entry("PYTHONUTF8".to_string())
            .or_insert_with(|| "1".to_string());
        env_map
            .entry("PYTHONIOENCODING".to_string())
            .or_insert_with(|| "utf-8".to_string());
        env_map = self.portable_env(env_map);
        self.sanitize_codex_env(&mut env_map);
        codex::codex_env(self.workspace.root_dir(), Some(&env_map))
    }

    pub fn tools_env(&self) -> HashMap<String, String> {
        let mut env_map: HashMap<String, String> = std::env::vars().collect();
        env_map
            .entry("PYTHONUTF8".to_string())
            .or_insert_with(|| "1".to_string());
        env_map
            .entry("PYTHONIOENCODING".to_string())
            .or_insert_with(|| "utf-8".to_string());
        env_map = self.portable_env(env_map);
        build_tools_env(self.workspace.root_dir(), Some(&env_map))
    }

    pub fn wheelhouse_path(&self) -> Option<PathBuf> {
        self.workspace.wheelhouse_path()
    }

    pub fn record_issue(
        &mut self,
        niveau: &str,
        message: &str,
        contexte: &str,
        details: Option<&str>,
    ) {
        let fingerprint = format!(
            "{niveau}|{contexte}|{message}|{}",
            details.unwrap_or_default()
        );
        if self.last_issue_fingerprint.as_deref() == Some(&fingerprint) {
            return;
        }
        self.last_issue_fingerprint = Some(fingerprint);

        let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let mut lines = vec![
            format!("## {timestamp}"),
            format!("- niveau: {niveau}"),
            format!("- contexte: {contexte}"),
            format!("- message: {message}"),
        ];
        if let Some(details) = details {
            lines.push(format!("- details: {details}"));
        }
        lines.push(String::new());

        let content = lines.join("\n");
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.workspace.bug_log_path())
            .and_then(|mut file| file.write_all(content.as_bytes()));
    }

    pub fn ensure_node_available_message(
        &self,
        env_map: &HashMap<String, String>,
    ) -> Option<String> {
        if node_executable(self.workspace.root_dir(), Some(env_map)).is_some() {
            return None;
        }

        let expected = self.workspace.tools_node().display();
        let portable_codex_present =
            codex_entrypoint_js(&codex_install_prefix(self.workspace.root_dir())).is_some();
        if portable_codex_present {
            return Some(format!(
                "Codex est installe mais Node portable introuvable. Place node dans {expected} (ex: node.exe). Fallback Node hote possible via USBIDE_CODEX_ALLOW_HOST_NODE=1."
            ));
        }

        Some(format!(
            "Node portable introuvable. Place node dans {expected} (ex: node.exe). Fallback Node hote possible via USBIDE_CODEX_ALLOW_HOST_NODE=1."
        ))
    }

    pub fn codex_device_auth_enabled(&self) -> bool {
        std::env::var("USBIDE_CODEX_DEVICE_AUTH")
            .map(|v| truthy(Some(&v)))
            .unwrap_or(false)
    }

    pub fn codex_auto_install_enabled(&self) -> bool {
        std::env::var("USBIDE_CODEX_AUTO_INSTALL")
            .map(|v| {
                !matches!(
                    v.trim().to_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                )
            })
            .unwrap_or(true)
    }
}

pub fn codex_exec_extra_args(
    sandbox_supported: Option<bool>,
    sandbox_mode: CodexSandboxMode,
    approval_supported: Option<bool>,
    approval_policy: CodexApprovalPolicy,
) -> Vec<String> {
    let mut args = Vec::new();
    if sandbox_supported != Some(false) {
        args.push("--sandbox".to_string());
        args.push(sandbox_mode.as_str().to_string());
    }
    if approval_supported != Some(false) {
        args.push("--ask-for-approval".to_string());
        args.push(approval_policy.as_str().to_string());
    }
    args
}

pub fn codex_sandbox_label(mode: CodexSandboxMode) -> &'static str {
    match mode {
        CodexSandboxMode::ReadOnly => "lecture seule",
        CodexSandboxMode::WorkspaceWrite => "agent (workspace)",
        CodexSandboxMode::DangerFullAccess => "danger (acces complet)",
    }
}

pub fn codex_approval_label(policy: CodexApprovalPolicy) -> &'static str {
    match policy {
        CodexApprovalPolicy::Untrusted => "non fiable",
        CodexApprovalPolicy::OnFailure => "sur echec",
        CodexApprovalPolicy::OnRequest => "sur demande",
        CodexApprovalPolicy::Never => "jamais",
    }
}

pub fn next_codex_sandbox_mode(mode: CodexSandboxMode) -> CodexSandboxMode {
    match mode {
        CodexSandboxMode::ReadOnly => CodexSandboxMode::WorkspaceWrite,
        CodexSandboxMode::WorkspaceWrite => CodexSandboxMode::DangerFullAccess,
        CodexSandboxMode::DangerFullAccess => CodexSandboxMode::ReadOnly,
    }
}

pub fn next_codex_approval_policy(policy: CodexApprovalPolicy) -> CodexApprovalPolicy {
    match policy {
        CodexApprovalPolicy::OnRequest => CodexApprovalPolicy::OnFailure,
        CodexApprovalPolicy::OnFailure => CodexApprovalPolicy::Untrusted,
        CodexApprovalPolicy::Untrusted => CodexApprovalPolicy::Never,
        CodexApprovalPolicy::Never => CodexApprovalPolicy::OnRequest,
    }
}

pub fn approval_flag_error(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("--ask-for-approval")
        && (lower.contains("unexpected argument")
            || lower.contains("unknown argument")
            || lower.contains("unrecognized"))
}

pub fn sandbox_flag_error(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("--sandbox")
        && (lower.contains("unexpected argument")
            || lower.contains("unknown argument")
            || lower.contains("unrecognized"))
}

pub fn sandbox_value_error(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("--sandbox")
        && (lower.contains("invalid value") || lower.contains("possible values"))
}

fn truthy(value: Option<&String>) -> bool {
    value
        .map(|v| v.trim().to_lowercase())
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::{CodexApprovalPolicy, CodexSandboxMode};
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_lock<F: FnOnce()>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        f();
    }

    fn set_env(key: &str, value: &str) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn portable_env_defauts() {
        let dir = TempDir::new().unwrap();
        let core = AppCore::new(dir.path().to_path_buf());
        let env = core.portable_env(HashMap::new());
        let root = core.workspace().root_dir();

        assert_eq!(
            env.get("CODEX_HOME").unwrap(),
            &root.join("codex_home").display().to_string()
        );
        assert_eq!(
            env.get("TEMP").unwrap(),
            &root.join("tmp").display().to_string()
        );
    }

    #[test]
    fn sanitize_codex_env_supprime_secrets() {
        let dir = TempDir::new().unwrap();
        let core = AppCore::new(dir.path().to_path_buf());

        with_env_lock(|| {
            let mut env = HashMap::from([
                ("OPENAI_API_KEY".to_string(), "sk-openai".to_string()),
                ("CODEX_API_KEY".to_string(), "sk-codex".to_string()),
                (
                    "OPENAI_BASE_URL".to_string(),
                    "https://example.com".to_string(),
                ),
            ]);
            remove_env("USBIDE_CODEX_ALLOW_API_KEY");
            remove_env("USBIDE_CODEX_ALLOW_CUSTOM_BASE");
            core.sanitize_codex_env(&mut env);

            assert!(!env.contains_key("OPENAI_API_KEY"));
            assert!(!env.contains_key("CODEX_API_KEY"));
            assert!(!env.contains_key("OPENAI_BASE_URL"));
        });
    }

    #[test]
    fn sanitize_codex_env_respecte_overrides() {
        let dir = TempDir::new().unwrap();
        let core = AppCore::new(dir.path().to_path_buf());

        with_env_lock(|| {
            let mut env = HashMap::from([
                ("OPENAI_API_KEY".to_string(), "sk-openai".to_string()),
                ("CODEX_API_KEY".to_string(), "sk-codex".to_string()),
                (
                    "OPENAI_BASE_URL".to_string(),
                    "https://example.com".to_string(),
                ),
            ]);
            set_env("USBIDE_CODEX_ALLOW_API_KEY", "1");
            set_env("USBIDE_CODEX_ALLOW_CUSTOM_BASE", "true");
            core.sanitize_codex_env(&mut env);

            assert_eq!(env.get("OPENAI_API_KEY").unwrap(), "sk-openai");
            assert_eq!(env.get("CODEX_API_KEY").unwrap(), "sk-codex");
            assert_eq!(env.get("OPENAI_BASE_URL").unwrap(), "https://example.com");

            remove_env("USBIDE_CODEX_ALLOW_API_KEY");
            remove_env("USBIDE_CODEX_ALLOW_CUSTOM_BASE");
        });
    }

    #[test]
    fn record_issue_dedupplique() {
        let dir = TempDir::new().unwrap();
        let mut core = AppCore::new(dir.path().to_path_buf());

        core.record_issue("erreur", "Test", "unitaire", None);
        core.record_issue("erreur", "Test", "unitaire", None);

        let content = fs::read_to_string(dir.path().join("bug.md")).unwrap();
        assert_eq!(content.matches("message: Test").count(), 1);
    }

    #[test]
    fn flags_codex_depuis_env() {
        let dir = TempDir::new().unwrap();
        let core = AppCore::new(dir.path().to_path_buf());

        with_env_lock(|| {
            set_env("USBIDE_CODEX_DEVICE_AUTH", "1");
            assert!(core.codex_device_auth_enabled());
            set_env("USBIDE_CODEX_AUTO_INSTALL", "0");
            assert!(!core.codex_auto_install_enabled());
            remove_env("USBIDE_CODEX_DEVICE_AUTH");
            remove_env("USBIDE_CODEX_AUTO_INSTALL");
        });
    }

    #[test]
    fn argv_codex_prennent_flags_supportes() {
        let args = codex_exec_extra_args(
            Some(true),
            CodexSandboxMode::WorkspaceWrite,
            Some(true),
            CodexApprovalPolicy::Never,
        );

        assert!(args.contains(&"--sandbox".to_string()));
        assert!(args.contains(&"workspace-write".to_string()));
        assert!(args.contains(&"--ask-for-approval".to_string()));
        assert!(args.contains(&"never".to_string()));
    }
}
