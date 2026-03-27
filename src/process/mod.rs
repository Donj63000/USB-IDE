use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::Path;
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

#[derive(Debug)]
pub struct ProcHandle {
    pub rx: Receiver<ProcEvent>,
    join: thread::JoinHandle<()>,
}

impl ProcHandle {
    pub fn join(self) {
        let _ = self.join.join();
    }
}

pub trait ProcessRunner {
    fn spawn(
        &self,
        argv: &[String],
        cwd: Option<&Path>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<ProcHandle, ProcessError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeProcessRunner;

impl ProcessRunner for NativeProcessRunner {
    fn spawn(
        &self,
        argv: &[String],
        cwd: Option<&Path>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<ProcHandle, ProcessError> {
        if argv.is_empty() {
            return Err(ProcessError::EmptyArgv);
        }

        let mut cmd = Command::new(&argv[0]);
        if argv.len() > 1 {
            cmd.args(&argv[1..]);
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        if let Some(env) = env {
            cmd.env_clear();
            for (key, value) in env {
                cmd.env(key, value);
            }
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(ProcessError::Spawn)?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::channel::<ProcEvent>();

        let join = thread::spawn(move || {
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
            let code = status.and_then(|status| status.code());
            let _ = tx.send(ProcEvent {
                kind: ProcEventKind::Exit,
                text: format!("exit {}", code.unwrap_or(-1)),
                returncode: code,
            });
        });

        Ok(ProcHandle { rx, join })
    }
}

/// Lance un subprocess et stream la sortie (stdout+stderr).
pub fn stream_subprocess(
    argv: &[String],
    cwd: Option<&Path>,
    env: Option<&HashMap<String, String>>,
) -> Result<ProcHandle, ProcessError> {
    NativeProcessRunner.spawn(argv, cwd, env)
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

    #[test]
    fn commande_invalide_declenche_erreur_spawn() {
        let argv = vec!["commande-introuvable-usbide-12345".to_string()];
        let res = stream_subprocess(&argv, None, None);
        assert!(matches!(res, Err(ProcessError::Spawn(_))));
    }

    #[test]
    fn stream_sortie_lignes() {
        let argv = if cfg!(windows) {
            vec![
                "cmd.exe".to_string(),
                "/d".to_string(),
                "/s".to_string(),
                "/c".to_string(),
                "echo bonjour".to_string(),
            ]
        } else {
            vec![
                "sh".to_string(),
                "-lc".to_string(),
                "printf 'bonjour\\n'".to_string(),
            ]
        };

        let handle = stream_subprocess(&argv, None, None).unwrap();
        let mut lines = Vec::new();
        while let Ok(event) = handle.rx.recv() {
            match event.kind {
                ProcEventKind::Line => lines.push(event.text),
                ProcEventKind::Exit => break,
            }
        }
        handle.join();

        assert!(lines.iter().any(|line| line.contains("bonjour")));
    }
}
