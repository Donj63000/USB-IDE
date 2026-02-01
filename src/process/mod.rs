use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcEventKind {
    Line,
    Exit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcEvent {
    pub kind: ProcEventKind,
    pub text: String,
    pub returncode: Option<i32>,
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("argv ne doit pas etre vide")]
    EmptyArgv,
    #[error("echec lancement process: {0}")]
    Spawn(#[from] io::Error),
}

pub struct ProcHandle {
    pub rx: Receiver<ProcEvent>,
    join: thread::JoinHandle<()>,
}

impl ProcHandle {
    pub fn join(self) {
        let _ = self.join.join();
    }
}

/// Lance un subprocess et stream la sortie (stdout+stderr).
pub fn stream_subprocess(
    argv: &[String],
    cwd: Option<&Path>,
    env: Option<&HashMap<String, String>>,
) -> Result<ProcHandle, ProcessError> {
    if argv.is_empty() {
        return Err(ProcessError::EmptyArgv);
    }
    let (tx, rx) = mpsc::channel::<ProcEvent>();
    let argv = argv.to_vec();
    let cwd = cwd.map(PathBuf::from);
    let env = env.cloned();

    let join = thread::spawn(move || {
        let mut cmd = Command::new(&argv[0]);
        if argv.len() > 1 {
            cmd.args(&argv[1..]);
        }
        if let Some(cwd) = cwd.as_ref() {
            cmd.current_dir(cwd);
        }
        if let Some(env) = env.as_ref() {
            cmd.env_clear();
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                let _ = tx.send(ProcEvent {
                    kind: ProcEventKind::Exit,
                    text: format!("exit -1 ({err})"),
                    returncode: None,
                });
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let mut handles = Vec::new();

        let spawn_reader = |stream: Box<dyn io::Read + Send>, tx: mpsc::Sender<ProcEvent>| {
            thread::spawn(move || {
                let mut reader = io::BufReader::new(stream);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            let text = line.trim_end_matches(['\n', '\r']).to_string();
                            let _ = tx.send(ProcEvent {
                                kind: ProcEventKind::Line,
                                text,
                                returncode: None,
                            });
                        }
                        Err(_) => break,
                    }
                }
            })
        };

        if let Some(out) = stdout {
            handles.push(spawn_reader(Box::new(out), tx.clone()));
        }
        if let Some(err) = stderr {
            handles.push(spawn_reader(Box::new(err), tx.clone()));
        }

        let status = child.wait().ok();
        for handle in handles {
            let _ = handle.join();
        }
        let code = status.and_then(|s| s.code());
        let _ = tx.send(ProcEvent {
            kind: ProcEventKind::Exit,
            text: format!("exit {}", code.unwrap_or(-1)),
            returncode: code,
        });
    });

    Ok(ProcHandle { rx, join })
}

/// Construit argv pour executer une commande via cmd.exe sur Windows.
pub fn windows_cmd_argv(command: &str) -> Vec<String> {
    let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
    vec![
        comspec,
        "/d".into(),
        "/s".into(),
        "/c".into(),
        command.to_string(),
    ]
}

/// Commande pour executer un script Python avec l'interpreteur courant (ou "python" par defaut).
pub fn python_run_argv(script: &Path) -> Vec<String> {
    let exe = std::env::var("USBIDE_PYTHON")
        .or_else(|_| std::env::var("PYTHON"))
        .unwrap_or_else(|_| "python".to_string());
    vec![exe, path_for_cmd(script)]
}

fn path_for_cmd(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    if !cfg!(windows) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_vide_declenche_erreur() {
        let res = stream_subprocess(&[], None, None);
        assert!(matches!(res, Err(ProcessError::EmptyArgv)));
    }
}
