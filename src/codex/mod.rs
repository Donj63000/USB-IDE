use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodexError {
    #[error("prompt ne doit pas etre vide")]
    EmptyPrompt,
    #[error("package ne doit pas etre vide")]
    EmptyPackage,
    #[error("tool ne doit pas etre vide")]
    EmptyTool,
    #[error("packages ne doit pas etre vide")]
    EmptyPackages,
    #[error("script ne doit pas etre vide")]
    EmptyScript,
    #[error("node portable introuvable")]
    NodeMissing,
    #[error("npm-cli.js introuvable")]
    NpmMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl CodexSandboxMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            CodexSandboxMode::ReadOnly => "read-only",
            CodexSandboxMode::WorkspaceWrite => "workspace-write",
            CodexSandboxMode::DangerFullAccess => "danger-full-access",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexApprovalPolicy {
    Untrusted,
    OnFailure,
    OnRequest,
    Never,
}

impl CodexApprovalPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            CodexApprovalPolicy::Untrusted => "untrusted",
            CodexApprovalPolicy::OnFailure => "on-failure",
            CodexApprovalPolicy::OnRequest => "on-request",
            CodexApprovalPolicy::Never => "never",
        }
    }
}

pub fn parse_codex_sandbox_mode(value: &str) -> Option<CodexSandboxMode> {
    match value.trim().to_lowercase().as_str() {
        "read-only" | "readonly" | "ro" => Some(CodexSandboxMode::ReadOnly),
        "workspace-write" | "workspace" | "write" | "agent" => {
            Some(CodexSandboxMode::WorkspaceWrite)
        }
        "danger-full-access" | "danger" | "full" | "full-access" => {
            Some(CodexSandboxMode::DangerFullAccess)
        }
        _ => None,
    }
}

pub fn parse_codex_approval_policy(value: &str) -> Option<CodexApprovalPolicy> {
    match value.trim().to_lowercase().as_str() {
        "untrusted" => Some(CodexApprovalPolicy::Untrusted),
        "on-failure" | "onfailure" => Some(CodexApprovalPolicy::OnFailure),
        "on-request" | "onrequest" => Some(CodexApprovalPolicy::OnRequest),
        "never" | "none" | "off" => Some(CodexApprovalPolicy::Never),
        _ => None,
    }
}

pub fn codex_sandbox_mode_from_env() -> CodexSandboxMode {
    env::var("USBIDE_CODEX_SANDBOX")
        .ok()
        .and_then(|v| parse_codex_sandbox_mode(&v))
        .unwrap_or(CodexSandboxMode::WorkspaceWrite)
}

pub fn codex_approval_policy_from_env() -> CodexApprovalPolicy {
    env::var("USBIDE_CODEX_APPROVAL")
        .ok()
        .and_then(|v| parse_codex_approval_policy(&v))
        .unwrap_or(CodexApprovalPolicy::Never)
}

pub fn translate_codex_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("--ask-for-approval")
        && (lower.contains("unexpected argument")
            || lower.contains("unknown argument")
            || lower.contains("unrecognized"))
    {
        return Some(
            "Erreur : l'option --ask-for-approval n'est pas reconnue par cette version de Codex."
                .to_string(),
        );
    }
    if lower.starts_with("tip:") && lower.contains("--ask-for-approval") {
        return Some(
            "Astuce : pour passer --ask-for-approval comme valeur, utilise -- --ask-for-approval."
                .to_string(),
        );
    }
    if lower.starts_with("usage: codex exec") {
        return Some(
            "Utilisation : codex exec --json --sandbox <MODE_SANDBOX> [PROMPT].".to_string(),
        );
    }
    if lower.starts_with("for more information") || lower.contains("try '--help'") {
        return Some("Pour plus d'information, utilise --help.".to_string());
    }
    if lower.starts_with("error:") {
        if lower.contains("unexpected argument")
            || lower.contains("unknown argument")
            || lower.contains("unrecognized")
        {
            return Some("Erreur : option inconnue ou invalide. Consulte --help.".to_string());
        }
        return Some("Erreur : commande Codex invalide. Consulte --help.".to_string());
    }
    if lower.starts_with("logged in using") {
        return Some("Connecte avec ChatGPT.".to_string());
    }
    if lower.starts_with("up to date in") {
        return Some("A jour.".to_string());
    }
    None
}

fn is_windows() -> bool {
    cfg!(windows)
}

fn path_for_cmd(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    if !is_windows() {
        return raw;
    }
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", stripped);
    }
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        return stripped.to_string();
    }
    raw
}

fn find_in_path(cmd: &str, path: Option<&str>, is_windows: bool) -> Option<PathBuf> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }

    let candidate = Path::new(cmd);
    let has_separator =
        cmd.contains(std::path::MAIN_SEPARATOR) || (is_windows && cmd.contains('/'));
    if candidate.is_absolute() || has_separator {
        return if candidate.exists() {
            Some(candidate.to_path_buf())
        } else {
            None
        };
    }

    let raw_path = path.unwrap_or_default();
    let path_iter = env::split_paths(raw_path);

    let mut extensions: Vec<String> = Vec::new();
    if is_windows && candidate.extension().is_none() {
        let pathext =
            env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD;.PS1".to_string());
        extensions = pathext
            .split(';')
            .filter(|ext| !ext.is_empty())
            .map(|ext| ext.to_string())
            .collect();
        if extensions.is_empty() {
            extensions.push(String::new());
        }
    } else {
        extensions.push(String::new());
    }

    for dir in path_iter {
        for ext in &extensions {
            let file_name = if ext.is_empty() {
                cmd.to_string()
            } else {
                format!("{cmd}{ext}")
            };
            let candidate = dir.join(file_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn env_value_from_map(
    env_map: Option<&HashMap<String, String>>,
    key: &str,
    is_windows: bool,
) -> Option<String> {
    env_map.and_then(|env| {
        env.get(key).cloned().or_else(|| {
            if is_windows {
                env.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(key))
                    .map(|(_, v)| v.clone())
            } else {
                None
            }
        })
    })
}

fn env_path_from_map(
    env_map: Option<&HashMap<String, String>>,
    is_windows: bool,
) -> Option<String> {
    env_value_from_map(env_map, "PATH", is_windows)
}

pub fn resolve_in_path(cmd: &str, env_map: &HashMap<String, String>) -> Option<PathBuf> {
    let path_value = env_map.get("PATH").cloned().or_else(|| {
        if is_windows() {
            env_map
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
                .map(|(_, v)| v.clone())
        } else {
            None
        }
    });
    find_in_path(cmd, path_value.as_deref(), is_windows())
}

// =============================================================================
// Outils Python (pip --prefix) : PyInstaller + outils dev
// =============================================================================

pub fn tools_install_prefix(root_dir: &Path) -> PathBuf {
    root_dir.join(".usbide").join("tools")
}

pub fn python_scripts_dir(prefix: &Path) -> PathBuf {
    if is_windows() {
        prefix.join("Scripts")
    } else {
        prefix.join("bin")
    }
}

pub fn tools_env(
    root_dir: &Path,
    base_env: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut env_map = base_env.cloned().unwrap_or_else(|| env::vars().collect());
    normalize_path_key(&mut env_map);
    let bin_dir = python_scripts_dir(&tools_install_prefix(root_dir));
    prepend_path(&mut env_map, &bin_dir);
    env_map
}

pub fn parse_tool_list(raw: &str) -> Vec<String> {
    let items = raw.replace(',', " ");
    let mut seen = HashSet::new();
    let mut cleaned = Vec::new();
    for item in items.split_whitespace() {
        if !seen.insert(item.to_string()) {
            continue;
        }
        cleaned.push(item.to_string());
    }
    cleaned
}

pub fn tool_available(
    tool: &str,
    root_dir: Option<&Path>,
    env: Option<&HashMap<String, String>>,
) -> Result<bool, CodexError> {
    if tool.trim().is_empty() {
        return Err(CodexError::EmptyTool);
    }
    let search_env = if let Some(root) = root_dir {
        tools_env(root, env)
    } else {
        env.cloned().unwrap_or_else(|| env::vars().collect())
    };
    let path_value = search_env.get("PATH").map(String::as_str);
    Ok(find_in_path(tool, path_value, is_windows()).is_some())
}

pub fn pyinstaller_available(
    root_dir: Option<&Path>,
    env: Option<&HashMap<String, String>>,
) -> bool {
    tool_available("pyinstaller", root_dir, env).unwrap_or(false)
}

pub fn pip_install_argv(
    prefix: &Path,
    packages: &[String],
    find_links: Option<&Path>,
    no_index: bool,
) -> Result<Vec<String>, CodexError> {
    let cleaned: Vec<String> = packages
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return Err(CodexError::EmptyPackages);
    }
    let mut argv = vec![
        "python".to_string(),
        "-m".to_string(),
        "pip".to_string(),
        "install".to_string(),
        "--upgrade".to_string(),
        "--prefix".to_string(),
        path_for_cmd(prefix),
    ];
    if no_index {
        argv.push("--no-index".to_string());
    }
    if let Some(links) = find_links {
        argv.push("--find-links".to_string());
        argv.push(path_for_cmd(links));
    }
    argv.extend(cleaned);
    Ok(argv)
}

pub fn pyinstaller_install_argv(
    prefix: &Path,
    find_links: Option<&Path>,
    no_index: bool,
) -> Result<Vec<String>, CodexError> {
    let packages = vec!["pyinstaller".to_string()];
    pip_install_argv(prefix, &packages, find_links, no_index)
}

pub fn pyinstaller_build_argv(
    script: &Path,
    dist_dir: &Path,
    onefile: bool,
    work_dir: Option<&Path>,
    spec_dir: Option<&Path>,
) -> Result<Vec<String>, CodexError> {
    if script.as_os_str().is_empty() {
        return Err(CodexError::EmptyScript);
    }
    let mut argv = vec![
        "pyinstaller".to_string(),
        "--noconfirm".to_string(),
        "--onedir".to_string(),
        "--distpath".to_string(),
        path_for_cmd(dist_dir),
    ];
    if onefile {
        argv.retain(|arg| arg != "--onedir");
        argv.insert(1, "--onefile".to_string());
    }
    if let Some(work) = work_dir {
        argv.push("--workpath".to_string());
        argv.push(path_for_cmd(work));
    }
    if let Some(spec) = spec_dir {
        argv.push("--specpath".to_string());
        argv.push(path_for_cmd(spec));
    }
    argv.push(path_for_cmd(script));
    Ok(argv)
}

// =============================================================================
// Codex CLI officiel (npm: @openai/codex)
// =============================================================================

pub fn codex_install_prefix(root_dir: &Path) -> PathBuf {
    root_dir.join(".usbide").join("codex")
}

pub fn codex_bin_dir(prefix: &Path) -> PathBuf {
    prefix.join("node_modules").join(".bin")
}

pub fn node_tools_dir(root_dir: &Path) -> PathBuf {
    root_dir.join("tools").join("node")
}

pub fn node_executable(
    root_dir: &Path,
    env_map: Option<&HashMap<String, String>>,
) -> Option<PathBuf> {
    node_executable_with_os(root_dir, env_map, is_windows())
}

fn node_executable_with_os(
    root_dir: &Path,
    env_map: Option<&HashMap<String, String>>,
    is_windows: bool,
) -> Option<PathBuf> {
    let node_dir = node_tools_dir(root_dir);
    let mut candidates = Vec::new();
    if is_windows {
        candidates.push(node_dir.join("node.exe"));
    } else {
        candidates.push(node_dir.join("bin").join("node"));
        candidates.push(node_dir.join("node"));
    }

    let path_value = env_map
        .and_then(|env| env_path_from_map(Some(env), is_windows))
        .or_else(|| env::var("PATH").ok());

    if let Some(path) = path_value.as_deref() {
        if let Some(found) = find_in_path("node", Some(path), is_windows) {
            candidates.push(found);
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

pub fn npm_cli_js(root_dir: &Path, node: Option<&Path>) -> Option<PathBuf> {
    let node_path = match node {
        Some(node) => node.to_path_buf(),
        None => node_executable(root_dir, None)?,
    };
    let node_dir = node_path.parent()?;
    let candidate = node_dir
        .join("node_modules")
        .join("npm")
        .join("bin")
        .join("npm-cli.js");
    if candidate.exists() {
        return Some(candidate);
    }

    for alt in [
        node_dir
            .parent()
            .unwrap_or(node_dir)
            .join("lib")
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js"),
        node_dir
            .join("..")
            .join("lib")
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js"),
    ] {
        if let Ok(resolved) = alt.canonicalize() {
            if resolved.exists() {
                return Some(resolved);
            }
        }
    }
    None
}

pub fn codex_env(
    root_dir: &Path,
    base_env: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    let mut env_map = base_env.cloned().unwrap_or_else(|| env::vars().collect());
    normalize_path_key(&mut env_map);
    let node_dir = node_tools_dir(root_dir);
    let node_bin = node_dir.join("bin");
    let bin_dir = codex_bin_dir(&codex_install_prefix(root_dir));
    if node_bin.exists() {
        prepend_path(&mut env_map, &node_bin);
    }
    prepend_path(&mut env_map, &node_dir);
    prepend_path(&mut env_map, &bin_dir);
    env_map
}

pub fn codex_package_json(prefix: &Path) -> PathBuf {
    prefix
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("package.json")
}

pub fn codex_entrypoint_js(prefix: &Path) -> Option<PathBuf> {
    let pkg_json = codex_package_json(prefix);
    if !pkg_json.exists() {
        return None;
    }
    let data: Value = serde_json::from_str(&std::fs::read_to_string(&pkg_json).ok()?).ok()?;
    let bin_field = data.get("bin")?;
    let mut rel: Option<String> = None;
    if let Some(bin) = bin_field.as_str() {
        rel = Some(bin.to_string());
    } else if let Some(obj) = bin_field.as_object() {
        if let Some(val) = obj.get("codex").and_then(|v| v.as_str()) {
            rel = Some(val.to_string());
        } else {
            for value in obj.values() {
                if let Some(val) = value.as_str() {
                    rel = Some(val.to_string());
                    break;
                }
            }
        }
    }
    let rel = rel?;
    let entry = pkg_json.parent()?.join(rel);
    if entry.exists() { Some(entry) } else { None }
}

fn codex_path_needs_node(path: &Path, is_windows: bool) -> bool {
    if !is_windows {
        return false;
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "cmd" | "bat" | "ps1")
}

pub fn codex_cli_available(
    root_dir: Option<&Path>,
    env_map: Option<&HashMap<String, String>>,
) -> bool {
    if let Some(root) = root_dir {
        let node = node_executable(root, env_map);
        let entry = codex_entrypoint_js(&codex_install_prefix(root));
        if node.is_some() && entry.is_some() {
            return true;
        }
    }
    let search_env = if let Some(root) = root_dir {
        codex_env(root, env_map)
    } else {
        env_map.cloned().unwrap_or_else(|| env::vars().collect())
    };
    let path_value = search_env.get("PATH").map(String::as_str);
    let resolved = find_in_path("codex", path_value, is_windows());
    if let Some(resolved) = resolved {
        if codex_path_needs_node(&resolved, is_windows()) {
            if let Some(root) = root_dir {
                return node_executable(root, Some(&search_env)).is_some();
            }
            return find_in_path("node", path_value, is_windows()).is_some();
        }
        return true;
    }
    false
}

fn codex_base_argv_with_os(
    root_dir: Option<&Path>,
    env_map: Option<&HashMap<String, String>>,
    is_windows: bool,
) -> Vec<String> {
    if let Some(root) = root_dir {
        let node = node_executable_with_os(root, env_map, is_windows);
        let entry = codex_entrypoint_js(&codex_install_prefix(root));
        if let (Some(node), Some(entry)) = (node, entry) {
            return vec![path_for_cmd(&node), path_for_cmd(&entry)];
        }
    }

    if is_windows {
        let search_path = env_path_from_map(env_map, is_windows).or_else(|| env::var("PATH").ok());
        let resolved = find_in_path("codex", search_path.as_deref(), is_windows);
        if let Some(resolved) = resolved {
            let suffix = resolved
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_lowercase();
            if suffix == "cmd" || suffix == "bat" {
                let comspec = env_value_from_map(env_map, "COMSPEC", is_windows)
                    .or_else(|| env::var("COMSPEC").ok())
                    .unwrap_or_else(|| "cmd.exe".to_string());
                return vec![
                    comspec,
                    "/d".into(),
                    "/s".into(),
                    "/c".into(),
                    path_for_cmd(&resolved),
                ];
            }
            if suffix == "ps1" {
                let powershell = find_in_path("powershell", search_path.as_deref(), is_windows)
                    .unwrap_or_else(|| PathBuf::from("powershell"));
                return vec![
                    powershell.to_string_lossy().to_string(),
                    "-NoProfile".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-File".into(),
                    path_for_cmd(&resolved),
                ];
            }
            return vec![path_for_cmd(&resolved)];
        }
    }

    vec!["codex".to_string()]
}

pub fn codex_login_argv(
    root_dir: Option<&Path>,
    env_map: Option<&HashMap<String, String>>,
    device_auth: bool,
) -> Vec<String> {
    let mut argv = codex_base_argv_with_os(root_dir, env_map, is_windows());
    argv.push("login".to_string());
    if device_auth {
        argv.push("--device-auth".to_string());
    }
    argv
}

pub fn codex_status_argv(
    root_dir: Option<&Path>,
    env_map: Option<&HashMap<String, String>>,
) -> Vec<String> {
    let mut argv = codex_base_argv_with_os(root_dir, env_map, is_windows());
    argv.push("login".to_string());
    argv.push("status".to_string());
    argv
}

pub fn codex_exec_argv(
    prompt: &str,
    root_dir: Option<&Path>,
    env_map: Option<&HashMap<String, String>>,
    json_output: bool,
    extra_args: Option<&[String]>,
) -> Result<Vec<String>, CodexError> {
    if prompt.trim().is_empty() {
        return Err(CodexError::EmptyPrompt);
    }
    let mut argv = codex_base_argv_with_os(root_dir, env_map, is_windows());
    argv.push("exec".to_string());
    if json_output {
        argv.push("--json".to_string());
    }
    if let Some(extra) = extra_args {
        for arg in extra {
            if !arg.trim().is_empty() {
                argv.push(arg.clone());
            }
        }
    }
    let trimmed_prompt = prompt.trim_start();
    if trimmed_prompt.starts_with('-') {
        argv.push("--".to_string());
    }
    argv.push(prompt.to_string());
    Ok(argv)
}

pub fn codex_install_argv(
    root_dir: &Path,
    prefix: &Path,
    package: &str,
) -> Result<Vec<String>, CodexError> {
    if package.trim().is_empty() {
        return Err(CodexError::EmptyPackage);
    }
    let node = node_executable(root_dir, None).ok_or(CodexError::NodeMissing)?;
    let npm = npm_cli_js(root_dir, Some(&node)).ok_or(CodexError::NpmMissing)?;
    Ok(vec![
        path_for_cmd(&node),
        path_for_cmd(&npm),
        "install".to_string(),
        "--prefix".to_string(),
        path_for_cmd(prefix),
        "--no-audit".to_string(),
        "--no-fund".to_string(),
        package.to_string(),
    ])
}

fn prepend_path(env_map: &mut HashMap<String, String>, path: &Path) {
    normalize_path_key(env_map);
    let path_str = path.to_string_lossy();
    let current = env_map.get("PATH").cloned().unwrap_or_default();
    let mut paths: Vec<PathBuf> = env::split_paths(&current).collect();
    if !paths.iter().any(|p| p == path) {
        paths.insert(0, path.to_path_buf());
    }
    if let Ok(joined) = env::join_paths(paths) {
        env_map.insert("PATH".to_string(), joined.to_string_lossy().to_string());
    } else if current.is_empty() {
        env_map.insert("PATH".to_string(), path_str.to_string());
    } else {
        let sep = if is_windows() { ";" } else { ":" };
        env_map.insert("PATH".to_string(), format!("{path_str}{sep}{current}"));
    }
}

fn normalize_path_key(env_map: &mut HashMap<String, String>) {
    if !is_windows() {
        return;
    }
    if env_map.contains_key("PATH") {
        return;
    }
    let mut found: Option<(String, String)> = None;
    for (key, value) in env_map.iter() {
        if key.eq_ignore_ascii_case("PATH") {
            found = Some((key.clone(), value.clone()));
            break;
        }
    }
    if let Some((key, value)) = found {
        env_map.remove(&key);
        env_map.insert("PATH".to_string(), value);
    }
}

// =============================================================================
// Parsing JSONL Codex (affichage compact)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DisplayKind {
    Assistant,
    User,
    Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisplayItem {
    pub kind: DisplayKind,
    pub message: String,
}

pub fn extract_status_code(msg: &str) -> Option<u16> {
    let re = Regex::new(r"(?i)(?:unexpected status|last status[: ]+)\s*(\d{3})").ok()?;
    if let Some(cap) = re.captures(msg) {
        return cap.get(1).and_then(|m| m.as_str().parse::<u16>().ok());
    }
    let re_any = Regex::new(r"\b(\d{3})\b").ok()?;
    re_any
        .captures(msg)
        .and_then(|cap| cap.get(1))
        .and_then(|m| m.as_str().parse::<u16>().ok())
}

pub fn codex_hint_for_status(status: u16) -> Option<String> {
    match status {
        401 => Some(
            "401 = authentification invalide -> Ctrl+K (login) ou `codex logout` + login ChatGPT."
                .to_string(),
        ),
        403 => Some(
            "403 = acces interdit -> verifie login ChatGPT (pas API key) / droits / reseau."
                .to_string(),
        ),
        407 => Some("407 = proxy auth required -> configure HTTP_PROXY/HTTPS_PROXY.".to_string()),
        429 => Some("429 = rate limit -> reessaie plus tard / ralentis.".to_string()),
        500..=599 => {
            Some("5xx = erreur serveur -> reessaie, possible incident cote OpenAI.".to_string())
        }
        _ => None,
    }
}

fn extract_text_from_content(content: &Value) -> Vec<String> {
    let mut texts = Vec::new();
    match content {
        Value::Array(items) => {
            for item in items {
                if let Value::Object(map) = item {
                    if let Some(Value::String(item_type)) = map.get("type") {
                        if ["output_text", "output_markdown", "text", "input_text"]
                            .contains(&item_type.as_str())
                        {
                            if let Some(Value::String(text)) =
                                map.get("text").or_else(|| map.get("content"))
                            {
                                if !text.is_empty() {
                                    texts.push(text.clone());
                                }
                            }
                        }
                    }
                } else if let Value::String(text) = item {
                    texts.push(text.clone());
                }
            }
        }
        Value::String(text) => texts.push(text.clone()),
        _ => {}
    }
    texts
}

fn push_item(items: &mut Vec<DisplayItem>, kind: DisplayKind, msg: &Value) {
    if let Value::String(text) = msg {
        if !text.is_empty() {
            items.push(DisplayItem {
                kind,
                message: text.clone(),
            });
        }
    }
}

fn items_from_message_payload(payload: &Value) -> Vec<DisplayItem> {
    let mut items = Vec::new();
    let payload_type = payload.get("type").and_then(Value::as_str);
    if payload_type != Some("message") {
        return items;
    }
    let role = payload.get("role").and_then(Value::as_str);
    let kind = match role {
        Some("assistant") => DisplayKind::Assistant,
        Some("user") => DisplayKind::User,
        _ => return items,
    };

    let content = payload.get("content").unwrap_or(&Value::Null);
    let texts = extract_text_from_content(content);
    if !texts.is_empty() {
        for text in texts {
            items.push(DisplayItem {
                kind: kind.clone(),
                message: text,
            });
        }
        return items;
    }

    if let Some(Value::String(msg)) = payload.get("message") {
        if !msg.is_empty() {
            items.push(DisplayItem {
                kind,
                message: msg.clone(),
            });
        }
    }
    items
}

fn items_from_item_payload(item: &Value) -> Vec<DisplayItem> {
    let mut items = Vec::new();
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");

    if item_type == "message" {
        return items_from_message_payload(item);
    }

    if item_type == "agent_message" || item_type == "assistant_message" {
        for text in extract_text_from_content(item.get("content").unwrap_or(&Value::Null)) {
            items.push(DisplayItem {
                kind: DisplayKind::Assistant,
                message: text,
            });
        }
        push_item(
            &mut items,
            DisplayKind::Assistant,
            item.get("text").unwrap_or(&Value::Null),
        );
        push_item(
            &mut items,
            DisplayKind::Assistant,
            item.get("message").unwrap_or(&Value::Null),
        );
        return items;
    }

    if item_type == "user_message" || item_type == "user" {
        for text in extract_text_from_content(item.get("content").unwrap_or(&Value::Null)) {
            items.push(DisplayItem {
                kind: DisplayKind::User,
                message: text,
            });
        }
        push_item(
            &mut items,
            DisplayKind::User,
            item.get("text").unwrap_or(&Value::Null),
        );
        push_item(
            &mut items,
            DisplayKind::User,
            item.get("message").unwrap_or(&Value::Null),
        );
    }
    items
}

fn iter_tool_calls(containers: &[&Value]) -> Vec<Value> {
    let mut calls = Vec::new();
    for container in containers {
        if let Value::Object(map) = container {
            if let Some(tool_call) = map.get("tool_call") {
                if tool_call.is_object() {
                    calls.push(tool_call.clone());
                }
            }
            if let Some(Value::Array(list)) = map.get("tool_calls").or_else(|| map.get("tools")) {
                for call in list {
                    if call.is_object() {
                        calls.push(call.clone());
                    }
                }
            }
        }
    }
    calls
}

fn format_action(payload: &Value) -> Option<String> {
    let raw_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let is_action = matches!(
        raw_type.as_str(),
        "tool_call" | "function_call" | "action" | "tool"
    );
    if !is_action {
        let has_name = payload.get("name").is_some()
            || payload.get("tool").is_some()
            || payload.get("tool_name").is_some();
        let has_args = payload.get("arguments").is_some()
            || payload.get("args").is_some()
            || payload.get("input").is_some()
            || payload.get("parameters").is_some();
        if !(has_name && has_args) {
            return None;
        }
    }

    let name = payload
        .get("name")
        .or_else(|| payload.get("tool"))
        .or_else(|| payload.get("tool_name"))
        .or_else(|| payload.get("id"));
    let args = payload
        .get("arguments")
        .or_else(|| payload.get("args"))
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("parameters"));

    let description = payload
        .get("message")
        .or_else(|| payload.get("description"));
    if let Some(Value::String(desc)) = description {
        if !desc.trim().is_empty() && name.is_none() && args.is_none() {
            return Some(desc.trim().to_string());
        }
    }

    let arg_text = args.map(|val| {
        if val.is_object() || val.is_array() {
            serde_json::to_string(val).unwrap_or_else(|_| val.to_string())
        } else {
            val.to_string().trim_matches('"').to_string()
        }
    });

    match (name, arg_text) {
        (Some(Value::String(n)), Some(a)) if !n.is_empty() => Some(format!("{n}: {a}")),
        (Some(Value::String(n)), None) if !n.is_empty() => Some(n.clone()),
        (Some(n), Some(a)) => Some(format!("{}: {a}", n.to_string().trim_matches('"'))),
        (Some(n), None) => Some(n.to_string().trim_matches('"').to_string()),
        (None, Some(a)) if !a.is_empty() => Some(a),
        _ => None,
    }
}

pub fn extract_display_items(obj: &Value) -> Vec<DisplayItem> {
    let mut items = Vec::new();
    let event_type = obj.get("type").and_then(Value::as_str);
    let payload = obj.get("payload").unwrap_or(&Value::Null);

    if event_type == Some("event_msg") {
        if let Value::Object(map) = payload {
            let payload_type = map.get("type").and_then(Value::as_str);
            let msg = map
                .get("message")
                .or_else(|| map.get("text"))
                .unwrap_or(&Value::Null);
            match payload_type {
                Some("agent_message") | Some("assistant_message") => {
                    push_item(&mut items, DisplayKind::Assistant, msg);
                }
                Some("user_message") | Some("user") => {
                    push_item(&mut items, DisplayKind::User, msg);
                }
                _ => {
                    if let Some(action) = format_action(payload) {
                        items.push(DisplayItem {
                            kind: DisplayKind::Action,
                            message: action,
                        });
                    }
                }
            }
        }
    }

    if event_type == Some("response_item") {
        items.extend(items_from_message_payload(payload));
        if let Some(action) = format_action(payload) {
            items.push(DisplayItem {
                kind: DisplayKind::Action,
                message: action,
            });
        }
    }

    if matches!(
        event_type,
        Some("response.output_text.done") | Some("response.output_text")
    ) {
        push_item(
            &mut items,
            DisplayKind::Assistant,
            obj.get("text").unwrap_or(&Value::Null),
        );
    }

    let item = obj.get("item").unwrap_or(&Value::Null);
    if item.is_object() {
        items.extend(items_from_item_payload(item));
        if let Some(action) = format_action(item) {
            items.push(DisplayItem {
                kind: DisplayKind::Action,
                message: action,
            });
        }
    }

    for call in iter_tool_calls(&[obj, payload, item]) {
        if let Some(action) = format_action(&call) {
            items.push(DisplayItem {
                kind: DisplayKind::Action,
                message: action,
            });
        }
    }

    let mut uniques = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let key = (item.kind.clone(), item.message.clone());
        if seen.insert(key) {
            uniques.push(item);
        }
    }
    uniques
}

pub fn extract_assistant_messages(obj: &Value) -> Vec<String> {
    extract_display_items(obj)
        .into_iter()
        .filter(|item| item.kind == DisplayKind::Assistant)
        .map(|item| item.message)
        .collect()
}

pub fn hard_wrap(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = line.chars().collect();
    while start < chars.len() {
        let end = usize::min(start + width, chars.len());
        out.push(chars[start..end].iter().collect());
        start = end;
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if line.len() <= width {
        return vec![line.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        if current.is_empty() {
            if word.len() > width {
                lines.extend(hard_wrap(word, width));
            } else {
                current.push_str(word);
            }
            continue;
        }
        if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = String::new();
            if word.len() > width {
                lines.extend(hard_wrap(word, width));
            } else {
                current.push_str(word);
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(10);
    let mut lines = Vec::new();
    let mut in_code = false;
    for raw in text.lines() {
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            lines.push(raw.to_string());
            continue;
        }
        if in_code {
            if raw.len() <= width {
                lines.push(raw.to_string());
            } else {
                lines.extend(hard_wrap(raw, width));
            }
            continue;
        }
        if raw.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        lines.extend(wrap_line(raw, width));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_portable_node(root_dir: &Path) -> PathBuf {
        let node_dir = root_dir.join("tools").join("node");
        let node_path = if is_windows() {
            node_dir.join("node.exe")
        } else {
            node_dir.join("node")
        };
        fs::create_dir_all(node_path.parent().unwrap()).unwrap();
        fs::write(&node_path, "").unwrap();
        node_path
    }

    fn create_npm_cli(node_path: &Path) -> PathBuf {
        let npm_path = node_path
            .parent()
            .unwrap()
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js");
        fs::create_dir_all(npm_path.parent().unwrap()).unwrap();
        fs::write(&npm_path, "").unwrap();
        npm_path
    }

    fn create_codex_package(prefix: &Path) -> PathBuf {
        let pkg_dir = prefix.join("node_modules").join("@openai").join("codex");
        fs::create_dir_all(pkg_dir.join("bin")).unwrap();
        let entry_path = pkg_dir.join("bin").join("codex.js");
        fs::write(&entry_path, "").unwrap();
        let pkg_json = pkg_dir.join("package.json");
        fs::write(&pkg_json, r#"{"bin": {"codex": "bin/codex.js"}}"#).unwrap();
        entry_path
    }

    #[test]
    fn codex_login_argv_default() {
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), String::new());
        let argv = codex_login_argv(None, Some(&env_map), false);
        assert_eq!(argv[0], "codex");
        assert_eq!(&argv[1..], ["login"]);
    }

    #[test]
    fn codex_login_argv_device_auth() {
        let argv = codex_login_argv(None, None, true);
        assert!(argv.contains(&"--device-auth".to_string()));
    }

    #[test]
    fn codex_status_argv_default() {
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), String::new());
        let argv = codex_status_argv(None, Some(&env_map));
        assert_eq!(argv[0], "codex");
        assert_eq!(&argv[1..], ["login", "status"]);
    }

    #[test]
    fn codex_exec_argv_json() {
        let extra = vec!["--model".to_string(), "gpt-5".to_string()];
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), String::new());
        let argv = codex_exec_argv("hello", None, Some(&env_map), true, Some(&extra)).unwrap();
        assert_eq!(argv[0], "codex");
        assert!(argv.contains(&"--json".to_string()));
        assert!(argv.contains(&"--model".to_string()));
        assert!(argv.contains(&"gpt-5".to_string()));
        assert_eq!(argv.last().unwrap(), "hello");
    }

    #[test]
    fn codex_exec_argv_prompt_commence_par_tiret() {
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), String::new());
        let argv = codex_exec_argv("--help", None, Some(&env_map), true, None).unwrap();
        let pos = argv.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(argv[pos + 1], "--help");
    }

    #[test]
    fn codex_exec_argv_portable_prioritaire() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let node_path = create_portable_node(root);
        let entry_path = create_codex_package(&codex_install_prefix(root));
        let argv = codex_exec_argv("hello", Some(root), None, true, None).unwrap();
        assert_eq!(argv[0], node_path.to_string_lossy());
        let argv_entry = Path::new(&argv[1]);
        assert_eq!(
            argv_entry.components().collect::<Vec<_>>(),
            entry_path.components().collect::<Vec<_>>()
        );
        assert!(argv.contains(&"exec".to_string()));
    }

    #[test]
    fn codex_exec_argv_windows_cmd_shim() {
        let dir = TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let shim = bin_dir.join("codex.cmd");
        fs::write(&shim, "").unwrap();
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());
        env_map.insert("COMSPEC".to_string(), "cmd.exe".to_string());
        let argv = codex_base_argv_with_os(None, Some(&env_map), true);
        assert_eq!(argv[0], "cmd.exe");
        assert!(argv.contains(&"/c".to_string()));
        assert!(
            argv.iter()
                .any(|arg| arg.to_lowercase().ends_with("codex.cmd"))
        );
    }

    #[test]
    fn codex_exec_argv_rejecte_vide() {
        assert!(codex_exec_argv(" ", None, None, false, None).is_err());
    }

    #[test]
    fn codex_env_prepend_path() {
        let root_dir = Path::new("/tmp/usbide");
        let mut base = HashMap::new();
        base.insert("PATH".to_string(), "/bin".to_string());
        let env_map = codex_env(root_dir, Some(&base));
        let expected_bin = codex_bin_dir(&codex_install_prefix(root_dir))
            .to_string_lossy()
            .to_string();
        let expected_node = root_dir
            .join("tools")
            .join("node")
            .to_string_lossy()
            .to_string();
        let path = env_map.get("PATH").unwrap();
        assert!(path.starts_with(&expected_bin));
        assert!(path.contains(&expected_node));
        assert_eq!(base.get("PATH").unwrap(), "/bin");
    }

    #[test]
    fn codex_cli_available_fallback() {
        let dir = TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());
        if cfg!(windows) {
            let codex_bin = bin_dir.join("codex.cmd");
            fs::write(&codex_bin, "").unwrap();
            let node_bin = bin_dir.join("node.exe");
            fs::write(&node_bin, "").unwrap();
        } else {
            let codex_bin = bin_dir.join("codex");
            fs::write(&codex_bin, "").unwrap();
        }
        assert!(codex_cli_available(None, Some(&env_map)));
    }

    #[test]
    fn codex_cli_available_portable() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        create_portable_node(root);
        create_codex_package(&codex_install_prefix(root));
        assert!(codex_cli_available(Some(root), None));
    }

    #[test]
    #[cfg(windows)]
    fn codex_cli_available_cmd_requires_node() {
        let dir = TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let shim = bin_dir.join("codex.cmd");
        fs::write(&shim, "").unwrap();
        let mut env_map = HashMap::new();
        env_map.insert("PATH".to_string(), bin_dir.to_string_lossy().to_string());
        assert!(!codex_cli_available(None, Some(&env_map)));
        let node = bin_dir.join("node.exe");
        fs::write(&node, "").unwrap();
        assert!(codex_cli_available(None, Some(&env_map)));
    }

    #[test]
    fn node_executable_prefers_portable() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let node_path = create_portable_node(root);
        assert_eq!(node_executable(root, None).unwrap(), node_path);
    }

    #[test]
    fn npm_cli_js_finds() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let node_path = create_portable_node(root);
        let npm_path = create_npm_cli(&node_path);
        assert_eq!(npm_cli_js(root, Some(&node_path)).unwrap(), npm_path);
    }

    #[test]
    fn codex_entrypoint_js_resout() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let prefix = codex_install_prefix(root);
        let entry = create_codex_package(&prefix);
        assert_eq!(codex_entrypoint_js(&prefix).unwrap(), entry);
    }

    #[test]
    fn codex_install_argv_ok() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let node_path = create_portable_node(root);
        let npm_path = create_npm_cli(&node_path);
        let prefix = codex_install_prefix(root);
        let argv = codex_install_argv(root, &prefix, "@openai/codex").unwrap();
        assert!(argv.contains(&node_path.to_string_lossy().to_string()));
        assert!(argv.contains(&npm_path.to_string_lossy().to_string()));
        assert!(argv.contains(&"--prefix".to_string()));
        assert!(argv.contains(&prefix.to_string_lossy().to_string()));
    }

    #[test]
    fn codex_install_argv_rejecte_vide() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let prefix = codex_install_prefix(root);
        assert!(codex_install_argv(root, &prefix, " ").is_err());
    }

    #[test]
    fn parse_tool_list_ok() {
        let tools = parse_tool_list("ruff, black  mypy, pytest ruff");
        assert_eq!(tools, vec!["ruff", "black", "mypy", "pytest"]);
    }

    #[test]
    fn parse_codex_sandbox_mode_ok() {
        assert_eq!(
            parse_codex_sandbox_mode("read-only"),
            Some(CodexSandboxMode::ReadOnly)
        );
        assert_eq!(
            parse_codex_sandbox_mode("workspace-write"),
            Some(CodexSandboxMode::WorkspaceWrite)
        );
        assert_eq!(
            parse_codex_sandbox_mode("agent"),
            Some(CodexSandboxMode::WorkspaceWrite)
        );
        assert_eq!(
            parse_codex_sandbox_mode("danger-full-access"),
            Some(CodexSandboxMode::DangerFullAccess)
        );
        assert_eq!(parse_codex_sandbox_mode("???"), None);
    }

    #[test]
    fn parse_codex_approval_policy_ok() {
        assert_eq!(
            parse_codex_approval_policy("on-request"),
            Some(CodexApprovalPolicy::OnRequest)
        );
        assert_eq!(
            parse_codex_approval_policy("on-failure"),
            Some(CodexApprovalPolicy::OnFailure)
        );
        assert_eq!(
            parse_codex_approval_policy("untrusted"),
            Some(CodexApprovalPolicy::Untrusted)
        );
        assert_eq!(
            parse_codex_approval_policy("never"),
            Some(CodexApprovalPolicy::Never)
        );
        assert_eq!(parse_codex_approval_policy("???"), None);
    }

    #[test]
    fn translate_codex_line_ok() {
        assert!(
            translate_codex_line("error: unexpected argument '--ask-for-approval' found")
                .unwrap()
                .contains("option --ask-for-approval")
        );
        assert!(
            translate_codex_line(
                "tip: to pass '--ask-for-approval' as a value, use '-- --ask-for-approval'"
            )
            .unwrap()
            .starts_with("Astuce")
        );
        assert!(
            translate_codex_line("Usage: codex exec --json --sandbox <SANDBOX_MODE> [PROMPT]")
                .unwrap()
                .contains("Utilisation")
        );
        assert!(
            translate_codex_line("For more information, try '--help'.")
                .unwrap()
                .contains("--help")
        );
    }

    #[test]
    fn tool_available_rejecte_vide() {
        assert!(tool_available(" ", None, None).is_err());
    }

    #[test]
    fn tools_env_prepend_path() {
        let root_dir = Path::new("/tmp/usbide");
        let mut base = HashMap::new();
        base.insert("PATH".to_string(), "/bin".to_string());
        let env_map = tools_env(root_dir, Some(&base));
        let expected_bin = python_scripts_dir(&tools_install_prefix(root_dir))
            .to_string_lossy()
            .to_string();
        let path = env_map.get("PATH").unwrap();
        assert!(path.starts_with(&expected_bin));
        assert_eq!(base.get("PATH").unwrap(), "/bin");
    }

    #[test]
    fn pyinstaller_install_argv_ok() {
        let prefix = Path::new("/tmp/usbide/.usbide/tools");
        let argv = pyinstaller_install_argv(prefix, None, false).unwrap();
        assert!(argv.contains(&"--prefix".to_string()));
        assert!(argv.contains(&prefix.to_string_lossy().to_string()));
    }

    #[test]
    fn pip_install_argv_ok() {
        let prefix = Path::new("/tmp/usbide/.usbide/tools");
        let argv = pip_install_argv(
            prefix,
            &["ruff".to_string(), "black".to_string()],
            None,
            false,
        )
        .unwrap();
        assert!(argv.contains(&"--prefix".to_string()));
        assert!(argv.contains(&prefix.to_string_lossy().to_string()));
        assert!(argv.contains(&"ruff".to_string()));
        assert!(argv.contains(&"black".to_string()));
    }

    #[test]
    fn pip_install_argv_offline() {
        let prefix = Path::new("/tmp/usbide/.usbide/tools");
        let wheelhouse = Path::new("/tmp/usbide/tools/wheels");
        let argv = pip_install_argv(prefix, &["ruff".to_string()], Some(wheelhouse), true).unwrap();
        assert!(argv.contains(&"--no-index".to_string()));
        assert!(argv.contains(&"--find-links".to_string()));
        assert!(argv.contains(&wheelhouse.to_string_lossy().to_string()));
    }

    #[test]
    fn pip_install_argv_rejecte_vide() {
        let prefix = Path::new("/tmp/usbide/.usbide/tools");
        assert!(pip_install_argv(prefix, &[], None, false).is_err());
    }

    #[test]
    fn pyinstaller_build_argv_ok() {
        let script = Path::new("/tmp/usbide/app.py");
        let dist_dir = Path::new("/tmp/usbide/dist");
        let argv = pyinstaller_build_argv(script, dist_dir, true, None, None).unwrap();
        assert!(argv.contains(&script.to_string_lossy().to_string()));
        assert!(argv.contains(&dist_dir.to_string_lossy().to_string()));
        assert!(argv.contains(&"--onefile".to_string()));
        assert!(!argv.contains(&"--onedir".to_string()));
    }

    #[test]
    fn pyinstaller_build_argv_onedir_par_defaut() {
        let script = Path::new("/tmp/usbide/app.py");
        let dist_dir = Path::new("/tmp/usbide/dist");
        let argv = pyinstaller_build_argv(script, dist_dir, false, None, None).unwrap();
        assert!(argv.contains(&"--onedir".to_string()));
    }

    #[test]
    fn pyinstaller_build_argv_work_spec() {
        let script = Path::new("/tmp/usbide/app.py");
        let dist_dir = Path::new("/tmp/usbide/dist");
        let work_dir = Path::new("/tmp/usbide/build");
        let spec_dir = Path::new("/tmp/usbide");
        let argv = pyinstaller_build_argv(script, dist_dir, false, Some(work_dir), Some(spec_dir))
            .unwrap();
        assert!(argv.contains(&"--workpath".to_string()));
        assert!(argv.contains(&work_dir.to_string_lossy().to_string()));
        assert!(argv.contains(&"--specpath".to_string()));
        assert!(argv.contains(&spec_dir.to_string_lossy().to_string()));
    }

    #[test]
    fn pyinstaller_build_argv_rejecte_vide() {
        assert!(
            pyinstaller_build_argv(
                Path::new(""),
                Path::new("/tmp/usbide/dist"),
                false,
                None,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn extract_status_code_ok() {
        assert_eq!(
            extract_status_code("unexpected status 401 Unauthorized"),
            Some(401)
        );
        assert_eq!(extract_status_code("last status: 403 Forbidden"), Some(403));
        assert_eq!(extract_status_code("HTTP 429"), Some(429));
        assert_eq!(extract_status_code("aucun code ici"), None);
    }

    #[test]
    fn codex_hint_for_status_ok() {
        assert!(codex_hint_for_status(401).unwrap().contains("401"));
        assert!(codex_hint_for_status(403).unwrap().contains("403"));
        assert!(codex_hint_for_status(407).unwrap().contains("proxy"));
    }

    #[test]
    fn codex_extract_messages_response_item() {
        let obj: Value = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Bonjour"}]
            }
        });
        let items = extract_assistant_messages(&obj);
        assert_eq!(items, vec!["Bonjour".to_string()]);
    }

    #[test]
    fn codex_extract_messages_event_msg() {
        let obj: Value = serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "agent_message", "message": "Salut"}
        });
        let items = extract_assistant_messages(&obj);
        assert_eq!(items, vec!["Salut".to_string()]);
    }

    #[test]
    fn codex_extract_display_items_user() {
        let obj: Value = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Bonjour"}]
            }
        });
        let items = extract_display_items(&obj);
        assert!(
            items
                .iter()
                .any(|item| item.kind == DisplayKind::User && item.message == "Bonjour")
        );
    }

    #[test]
    fn codex_extract_display_items_action() {
        let obj: Value = serde_json::json!({
            "type": "response_item",
            "payload": {"type": "tool_call", "name": "list_files", "arguments": {"path": "."}}
        });
        let items = extract_display_items(&obj);
        assert!(items.iter().any(|item| item.kind == DisplayKind::Action && item.message.contains("list_files")));
    }

    #[test]
    fn codex_extract_display_items_item_completed() {
        let obj: Value = serde_json::json!({
            "type": "item.completed",
            "item": {"type": "agent_message", "text": "Salut"}
        });
        let items = extract_display_items(&obj);
        assert!(
            items
                .iter()
                .any(|item| item.kind == DisplayKind::Assistant && item.message == "Salut")
        );
    }

    #[test]
    fn codex_extract_text_filtre_types() {
        let content: Value = serde_json::json!([
            {"type": "output_text", "text": "OK"},
            {"type": "image", "text": "NO"}
        ]);
        let texts = extract_text_from_content(&content);
        assert_eq!(texts, vec!["OK".to_string()]);
    }

    #[test]
    fn wrap_text_wrappe() {
        let lines = wrap_text("Texte tres long avec des espaces pour verifier le wrap", 24);
        assert!(
            lines
                .iter()
                .all(|line| line.len() <= 24 || line.starts_with("```"))
        );
    }

    #[test]
    fn wrap_text_coupe_mot_long() {
        let lines = wrap_text("AAAAAAAAAAAAAAAAAAAA", 18);
        assert!(
            lines
                .iter()
                .all(|line| line.len() <= 18 || line.starts_with("```"))
        );
    }

    #[test]
    fn wrap_text_preserve_bloc_code() {
        let texte = "```python\nprint('x' * 50)\n```\nFin";
        let lines = wrap_text(texte, 20);
        assert!(lines.iter().any(|line| line.contains("print('x' * 50)")));
    }
}
