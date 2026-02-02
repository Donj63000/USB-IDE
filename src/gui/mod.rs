use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use eframe::egui::{self, Color32, RichText, ScrollArea, TextEdit};

use crate::codex::{
    CodexApprovalPolicy, CodexError, CodexSandboxMode, DisplayKind, codex_approval_policy_from_env,
    codex_cli_available, codex_entrypoint_js, codex_env, codex_exec_argv, codex_hint_for_status,
    codex_install_argv, codex_install_prefix, codex_login_argv, codex_sandbox_mode_from_env,
    codex_status_argv, extract_display_items, extract_status_code, node_executable,
    parse_tool_list, pip_install_argv, pyinstaller_available, pyinstaller_build_argv,
    pyinstaller_install_argv, resolve_in_path, tools_env, tools_install_prefix,
    translate_codex_line,
};
use crate::fs::{detect_text_encoding, is_probably_binary, read_text_with_encoding};
use crate::process::{
    ProcEventKind, ProcHandle, python_run_argv, stream_subprocess, windows_cmd_argv,
};

const APP_NAME: &str = "ValDev Pro v1";
const LOG_LIMIT: usize = 2000;

fn accent_red() -> Color32 {
    Color32::from_rgb(229, 57, 53)
}

fn accent_red_soft() -> Color32 {
    Color32::from_rgb(178, 45, 45)
}

fn panel_bg() -> Color32 {
    Color32::from_rgb(18, 22, 28)
}

fn panel_border() -> Color32 {
    Color32::from_rgb(46, 54, 66)
}

#[derive(Debug, Clone)]
struct OpenFile {
    path: PathBuf,
    encoding: String,
    dirty: bool,
}

#[derive(Debug, Clone, Copy)]
enum LogKind {
    Info,
    Warn,
    Error,
    User,
    Assistant,
    Action,
}

#[derive(Debug, Clone)]
struct LogLine {
    text: String,
    kind: LogKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogTarget {
    Main,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessKind {
    Shell,
    PythonRun,
    CodexExec,
    CodexLogin,
    CodexStatus,
    CodexInstall,
    DevTools,
    PyInstallerInstall,
    PyInstallerBuild,
}

struct RunningProcess {
    handle: ProcHandle,
    kind: ProcessKind,
    target: LogTarget,
    contexte: String,
}

#[derive(Debug, Clone)]
struct TreeEntry {
    path: PathBuf,
    name: String,
    depth: usize,
    is_dir: bool,
}

#[derive(Debug, Clone)]
struct FileNode {
    path: PathBuf,
    name: String,
    is_dir: bool,
    children: Vec<FileNode>,
}

struct FileTree {
    root: FileNode,
    expanded: HashSet<PathBuf>,
    visible: Vec<TreeEntry>,
    selected: Option<PathBuf>,
}

impl FileTree {
    fn new(root_dir: &Path) -> Self {
        let root = build_tree(root_dir);
        let mut expanded = HashSet::new();
        expanded.insert(root.path.clone());
        let mut tree = Self {
            root,
            expanded,
            visible: Vec::new(),
            selected: None,
        };
        tree.rebuild_visible();
        tree
    }

    fn rebuild_visible(&mut self) {
        self.visible.clear();
        let mut entries = Vec::new();
        flatten_tree(&self.root, 0, &self.expanded, &mut entries);
        self.visible = entries;
        if self.selected.is_none() {
            self.selected = self.visible.first().map(|entry| entry.path.clone());
        }
    }

    fn toggle_dir(&mut self, path: &Path) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
        }
        self.rebuild_visible();
    }
}

fn build_tree(path: &Path) -> FileNode {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string());
    let is_dir = path.is_dir();
    let mut children = Vec::new();
    if is_dir {
        if let Ok(read_dir) = std::fs::read_dir(path) {
            for entry in read_dir.flatten() {
                let child_path = entry.path();
                let child = build_tree(&child_path);
                children.push(child);
            }
            children.sort_by_key(|node| (!node.is_dir, node.name.to_lowercase()));
        }
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

pub fn run(root_dir: PathBuf) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]),
        ..Default::default()
    };
    let root = root_dir.clone();
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            configure_style(&cc.egui_ctx);
            Box::new(GuiApp::new(root))
        }),
    )
    .map_err(|err| anyhow::anyhow!("Erreur interface GUI: {err}"))?;
    Ok(())
}

fn configure_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(Color32::from_rgb(235, 238, 244));
    visuals.window_fill = Color32::from_rgb(12, 14, 18);
    visuals.panel_fill = Color32::from_rgb(14, 18, 24);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(18, 22, 28);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(26, 30, 38);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 30, 32);
    visuals.widgets.active.bg_fill = accent_red();
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
    visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
    visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
    visuals.widgets.active.rounding = egui::Rounding::same(6.0);
    visuals.selection.bg_fill = accent_red();
    visuals.selection.stroke.color = Color32::from_rgb(255, 192, 192);
    visuals.faint_bg_color = Color32::from_rgb(20, 24, 30);
    visuals.code_bg_color = Color32::from_rgb(16, 20, 26);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.interact_size = egui::vec2(36.0, 24.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(19.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(13.5, egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}

struct GuiApp {
    root_dir: PathBuf,
    current: Option<OpenFile>,
    editor_text: String,
    tree: FileTree,
    cmd_input: String,
    codex_input: String,
    log: Vec<LogLine>,
    codex_log: Vec<LogLine>,
    title: String,
    sub_title: String,
    running: Vec<RunningProcess>,
    bug_log_path: PathBuf,
    codex_compact_view: bool,
    codex_sandbox_mode: CodexSandboxMode,
    codex_approval_policy: CodexApprovalPolicy,
    codex_sandbox_supported: Option<bool>,
    codex_approval_supported: Option<bool>,
    codex_exec_used_sandbox_flag: bool,
    codex_exec_used_approval_flag: bool,
    codex_last_prompt: Option<String>,
    codex_retry_without_sandbox: bool,
    codex_retry_without_approval: bool,
    codex_log_buffer: String,
    codex_log_dirty: bool,
    last_codex_message: Option<String>,
    codex_assistant_buffer: String,
    codex_install_attempted: bool,
    pyinstaller_install_attempted: bool,
    pending_codex_prompt: Option<String>,
    last_window_title: String,
}

impl GuiApp {
    fn new(root_dir: PathBuf) -> Self {
        let root_dir = match root_dir.canonicalize() {
            Ok(path) => path,
            Err(_) => root_dir,
        };
        let bug_log_path = root_dir.join("bug.md");
        let tree = FileTree::new(&root_dir);
        let mut app = Self {
            root_dir,
            current: None,
            editor_text: String::new(),
            tree,
            cmd_input: String::new(),
            codex_input: String::new(),
            log: Vec::new(),
            codex_log: Vec::new(),
            title: APP_NAME.to_string(),
            sub_title: String::new(),
            running: Vec::new(),
            bug_log_path,
            codex_compact_view: true,
            codex_sandbox_mode: codex_sandbox_mode_from_env(),
            codex_approval_policy: codex_approval_policy_from_env(),
            codex_sandbox_supported: None,
            codex_approval_supported: None,
            codex_exec_used_sandbox_flag: false,
            codex_exec_used_approval_flag: false,
            codex_last_prompt: None,
            codex_retry_without_sandbox: false,
            codex_retry_without_approval: false,
            codex_log_buffer: String::new(),
            codex_log_dirty: true,
            last_codex_message: None,
            codex_assistant_buffer: String::new(),
            codex_install_attempted: false,
            pyinstaller_install_attempted: false,
            pending_codex_prompt: None,
            last_window_title: String::new(),
        };
        app.ensure_portable_dirs();
        app.refresh_title();
        app.log_ui(format!(
            "{APP_NAME}\nRoot: {}\nAstuce: lance la version TUI avec --ui tui si besoin.\n",
            app.root_dir.display()
        ));
        app.codex_log_ui(format!(
            "Sandbox Codex: {}",
            Self::codex_sandbox_label(app.codex_sandbox_mode)
        ));
        app.codex_log_ui(format!(
            "Approbations Codex: {}",
            Self::codex_approval_label(app.codex_approval_policy)
        ));
        app
    }
    fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = if self.sub_title.is_empty() {
            self.title.clone()
        } else {
            format!("{} - {}", self.title, self.sub_title)
        };
        if title != self.last_window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_window_title = title;
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.ctrl) {
            self.action_save();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.action_run();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
            self.action_clear_log();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
            self.action_reload_tree();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::K) && i.modifiers.ctrl) {
            self.action_codex_login();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl) {
            self.action_codex_check();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::I) && i.modifiers.ctrl) {
            self.action_codex_install();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::M) && i.modifiers.ctrl) {
            self.action_toggle_codex_view();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::E) && i.modifiers.ctrl) {
            self.action_build_exe();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
            self.action_dev_tools();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.ctrl) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn panel_frame(ui: &egui::Ui) -> egui::Frame {
        egui::Frame::group(ui.style())
            .fill(panel_bg())
            .stroke(egui::Stroke::new(1.0, panel_border()))
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::same(10.0))
    }

    fn toolbar_group<F: FnOnce(&mut egui::Ui)>(ui: &mut egui::Ui, add: F) {
        egui::Frame::none()
            .fill(Color32::from_rgb(20, 24, 30))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(40, 46, 58)))
            .rounding(egui::Rounding::same(6.0))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                    add(ui);
                });
            });
    }

    fn section_title(ui: &mut egui::Ui, label: &str) {
        ui.label(
            RichText::new(label)
                .strong()
                .color(Color32::from_rgb(235, 235, 240)),
        );
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let title = if self.current.as_ref().map(|f| f.dirty).unwrap_or(false) {
                format!("{APP_NAME} *")
            } else {
                APP_NAME.to_string()
            };
            ui.label(
                RichText::new(title)
                    .strong()
                    .size(20.0)
                    .color(Color32::from_rgb(245, 245, 250)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(&self.sub_title)
                    .color(Color32::from_gray(150))
                    .monospace(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Quitter").color(Color32::from_rgb(255, 220, 220)),
                        )
                        .fill(accent_red_soft()),
                    )
                    .clicked()
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            Self::toolbar_group(ui, |ui| {
                if ui.button("Sauver").clicked() {
                    self.action_save();
                }
                if ui.button("Executer (F5)").clicked() {
                    self.action_run();
                }
                if ui.button("Reload").clicked() {
                    self.action_reload_tree();
                }
                if ui.button("Vider logs").clicked() {
                    self.action_clear_log();
                }
            });
            Self::toolbar_group(ui, |ui| {
                if ui.button("Codex login").clicked() {
                    self.action_codex_login();
                }
                if ui.button("Codex status").clicked() {
                    self.action_codex_check();
                }
                if ui.button("Codex install").clicked() {
                    self.action_codex_install();
                }
                let mode_label = if self.codex_compact_view {
                    "Codex: Compact"
                } else {
                    "Codex: Brut"
                };
                if ui.button(mode_label).clicked() {
                    self.action_toggle_codex_view();
                }
            });
            Self::toolbar_group(ui, |ui| {
                if ui.button("Outils dev").clicked() {
                    self.action_dev_tools();
                }
                if ui.button("Build EXE").clicked() {
                    self.action_build_exe();
                }
            });
        });
    }

    fn draw_file_tree(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame(ui).show(ui, |ui| {
            Self::section_title(ui, "Fichiers");
            ui.separator();
            let entries = self.tree.visible.clone();
            let available_height = ui.available_height();
            ScrollArea::vertical()
                .id_source("file_tree")
                .auto_shrink([false, false])
                .max_height(available_height)
                .show(ui, |ui| {
                    for entry in entries {
                        let is_selected = self
                            .tree
                            .selected
                            .as_ref()
                            .map(|p| p == &entry.path)
                            .unwrap_or(false);
                        ui.horizontal(|ui| {
                            let indent = entry.depth as f32 * 12.0;
                            ui.add_space(indent);
                            if entry.is_dir {
                                let icon = if self.tree.expanded.contains(&entry.path) {
                                    "v"
                                } else {
                                    ">"
                                };
                                if ui.button(icon).clicked() {
                                    self.tree.toggle_dir(&entry.path);
                                }
                            } else {
                                ui.add_space(18.0);
                            }
                            let label = if entry.is_dir {
                                format!("{}/", entry.name)
                            } else {
                                entry.name.clone()
                            };
                            if ui.selectable_label(is_selected, label).clicked() {
                                self.tree.selected = Some(entry.path.clone());
                                if entry.is_dir {
                                    self.tree.toggle_dir(&entry.path);
                                } else {
                                    self.open_file(entry.path.clone());
                                }
                            }
                        });
                    }
                });
        });
    }

    fn draw_editor(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame(ui).show(ui, |ui| {
            if let Some(current) = &self.current {
                ui.horizontal(|ui| {
                    Self::section_title(ui, "Editeur");
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(current.path.display().to_string())
                            .color(Color32::from_gray(180)),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(current.encoding.clone()).color(Color32::from_gray(150)),
                    );
                    if current.dirty {
                        ui.add_space(10.0);
                        ui.colored_label(accent_red(), "modifie");
                    }
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);
                let available = ui.available_size();
                let editor = TextEdit::multiline(&mut self.editor_text)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .lock_focus(true);
                let response = ScrollArea::both()
                    .id_source("editor_scroll")
                    .auto_shrink([false, false])
                    .max_height(available.y)
                    .max_width(available.x)
                    .show(ui, |ui| {
                        ui.set_min_size(available);
                        ui.add_sized(available, editor)
                    })
                    .inner;
                if response.changed() {
                    if let Some(current) = self.current.as_mut() {
                        current.dirty = true;
                    }
                    self.refresh_title();
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(RichText::new("Aucun fichier ouvert.").heading());
                    ui.label(
                        RichText::new("Clique un fichier a gauche pour l'ouvrir.")
                            .color(Color32::from_gray(160)),
                    );
                });
            }
        });
    }

    fn draw_logs(&mut self, ui: &mut egui::Ui, target: LogTarget, id_source: &str) {
        let entries = match target {
            LogTarget::Main => &self.log,
            LogTarget::Codex => &self.codex_log,
        };
        ScrollArea::vertical()
            .id_source(id_source)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height(ui.available_height())
            .show(ui, |ui| {
                if entries.is_empty() {
                    ui.label(RichText::new("Aucun log.").color(Color32::from_gray(130)));
                }
                for entry in entries {
                    let color = match entry.kind {
                        LogKind::Info => Color32::from_gray(210),
                        LogKind::Warn => Color32::from_rgb(240, 200, 120),
                        LogKind::Error => Color32::from_rgb(240, 100, 100),
                        LogKind::User => Color32::from_rgb(120, 190, 255),
                        LogKind::Assistant => Color32::from_rgb(120, 220, 160),
                        LogKind::Action => Color32::from_rgb(218, 165, 72),
                    };
                    ui.label(RichText::new(&entry.text).color(color));
                }
            });
    }

    fn draw_command_panel(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame(ui).show(ui, |ui| {
            Self::section_title(ui, "Commande");
            ui.add_space(6.0);
            let mut submit = false;
            ui.horizontal(|ui| {
                let button_width = 90.0;
                let input_width =
                    (ui.available_width() - button_width - ui.spacing().item_spacing.x).max(140.0);
                let response = ui.add_sized(
                    [input_width, 0.0],
                    TextEdit::singleline(&mut self.cmd_input).hint_text("Ex: python script.py"),
                );
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if ui
                    .add_sized([button_width, 0.0], egui::Button::new("Executer"))
                    .clicked()
                {
                    submit = true;
                }
            });
            if submit {
                let cmd = self.cmd_input.trim().to_string();
                self.cmd_input.clear();
                self.run_shell(cmd);
            }
            ui.add_space(8.0);
            let log_height = ui.available_height().max(80.0);
            ui.allocate_ui(egui::vec2(ui.available_width(), log_height), |ui| {
                self.draw_logs(ui, LogTarget::Main, "log_main");
            });
        });
    }

    fn draw_codex_panel(&mut self, ui: &mut egui::Ui) {
        Self::panel_frame(ui).show(ui, |ui| {
            Self::section_title(ui, "Codex");
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Login").clicked() {
                    self.action_codex_login();
                }
                if ui.button("Status").clicked() {
                    self.action_codex_check();
                }
                if ui.button("Installer").clicked() {
                    self.action_codex_install();
                }
                let label = if self.codex_compact_view {
                    "Compact"
                } else {
                    "Brut"
                };
                if ui.button(label).clicked() {
                    self.action_toggle_codex_view();
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let sandbox_label = format!(
                    "Sandbox: {}",
                    Self::codex_sandbox_label(self.codex_sandbox_mode)
                );
                if ui.button(sandbox_label).clicked() {
                    self.action_toggle_codex_sandbox();
                }
                let approval_label = format!(
                    "Approvals: {}",
                    Self::codex_approval_label(self.codex_approval_policy)
                );
                if ui.button(approval_label).clicked() {
                    self.action_toggle_codex_approval();
                }
            });
            ui.add_space(4.0);
            let mut submit = false;
            ui.horizontal(|ui| {
                let button_width = 90.0;
                let input_width =
                    (ui.available_width() - button_width - ui.spacing().item_spacing.x).max(140.0);
                let response = ui.add_sized(
                    [input_width, 0.0],
                    TextEdit::singleline(&mut self.codex_input)
                        .hint_text("Ex: explique ce code..."),
                );
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if ui
                    .add_sized([button_width, 0.0], egui::Button::new("Envoyer"))
                    .clicked()
                {
                    submit = true;
                }
            });
            if submit {
                let prompt = self.codex_input.trim().to_string();
                self.codex_input.clear();
                self.run_codex(prompt);
            }
            ui.add_space(8.0);
            let log_height = ui.available_height().max(80.0);
            ui.allocate_ui(egui::vec2(ui.available_width(), log_height), |ui| {
                self.draw_codex_log(ui);
            });
        });
    }

    fn draw_codex_log(&mut self, ui: &mut egui::Ui) {
        if self.codex_log_dirty {
            self.codex_log_buffer = self.render_plain_log(&self.codex_log);
            self.codex_log_dirty = false;
        }
        let available = ui.available_size();
        let response = ui.add_sized(
            available,
            TextEdit::multiline(&mut self.codex_log_buffer)
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
        if response.changed() {
            self.codex_log_buffer = self.render_plain_log(&self.codex_log);
            self.codex_log_dirty = false;
        }
    }

    fn render_plain_log(&self, entries: &[LogLine]) -> String {
        let mut out = String::new();
        for (idx, entry) in entries.iter().enumerate() {
            out.push_str(&entry.text);
            if idx + 1 < entries.len() {
                out.push('\n');
            }
        }
        out
    }

    fn refresh_title(&mut self) {
        if let Some(current) = &self.current {
            let dirty = if current.dirty { " *" } else { "" };
            self.title = format!("{APP_NAME}{dirty}");
            self.sub_title = format!("{}  ({})", current.path.display(), current.encoding);
        } else {
            self.title = APP_NAME.to_string();
            self.sub_title = self.root_dir.display().to_string();
        }
    }

    fn push_log(&mut self, target: LogTarget, msg: String, kind: LogKind) {
        let lines: Vec<String> = msg.split('\n').map(|s| s.to_string()).collect();
        let store = match target {
            LogTarget::Main => &mut self.log,
            LogTarget::Codex => &mut self.codex_log,
        };
        for line in lines {
            store.push(LogLine { text: line, kind });
        }
        if store.len() > LOG_LIMIT {
            let drain = store.len() - LOG_LIMIT;
            store.drain(0..drain);
        }
        if matches!(target, LogTarget::Codex) {
            self.codex_log_dirty = true;
        }
    }

    fn log_ui(&mut self, msg: String) {
        self.push_log(LogTarget::Main, msg, LogKind::Info);
    }

    fn codex_log_ui(&mut self, msg: String) {
        self.push_log(LogTarget::Codex, msg, LogKind::Info);
    }

    fn log_issue(&mut self, msg: &str, niveau: &str, contexte: &str, target: LogTarget) {
        let kind = if niveau == "avertissement" {
            LogKind::Warn
        } else {
            LogKind::Error
        };
        self.push_log(target, msg.to_string(), kind);
        self.record_issue(niveau, msg, contexte, None);
    }

    fn record_issue(&self, niveau: &str, message: &str, contexte: &str, details: Option<&str>) {
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
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.bug_log_path)
            .and_then(|mut file| std::io::Write::write_all(&mut file, content.as_bytes()));
    }

    fn ensure_portable_dirs(&self) {
        for path in [
            self.root_dir.join("cache").join("pip"),
            self.root_dir.join("cache").join("pycache"),
            self.root_dir.join("cache").join("npm"),
            self.root_dir.join("tmp"),
            self.root_dir.join("codex_home"),
        ] {
            let _ = std::fs::create_dir_all(path);
        }
    }

    fn portable_env(&self, mut env_map: HashMap<String, String>) -> HashMap<String, String> {
        env_map.insert(
            "PIP_CACHE_DIR".to_string(),
            self.root_dir
                .join("cache")
                .join("pip")
                .display()
                .to_string(),
        );
        env_map.insert(
            "PYTHONPYCACHEPREFIX".to_string(),
            self.root_dir
                .join("cache")
                .join("pycache")
                .display()
                .to_string(),
        );
        env_map.insert(
            "TEMP".to_string(),
            self.root_dir.join("tmp").display().to_string(),
        );
        env_map.insert(
            "TMP".to_string(),
            self.root_dir.join("tmp").display().to_string(),
        );
        env_map.insert("PYTHONNOUSERSITE".to_string(), "1".to_string());
        env_map.insert(
            "CODEX_HOME".to_string(),
            self.root_dir.join("codex_home").display().to_string(),
        );
        env_map.insert(
            "NPM_CONFIG_CACHE".to_string(),
            self.root_dir
                .join("cache")
                .join("npm")
                .display()
                .to_string(),
        );
        env_map.insert(
            "NPM_CONFIG_UPDATE_NOTIFIER".to_string(),
            "false".to_string(),
        );
        env_map
    }

    fn truthy(value: Option<&String>) -> bool {
        value
            .map(|v| v.trim().to_lowercase())
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false)
    }

    fn sanitize_codex_env(&self, env_map: &mut HashMap<String, String>) {
        let allow_api_key = Self::truthy(std::env::var("USBIDE_CODEX_ALLOW_API_KEY").ok().as_ref());
        let allow_custom_base = Self::truthy(
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

    fn codex_env(&self) -> HashMap<String, String> {
        let mut env_map: HashMap<String, String> = std::env::vars().collect();
        env_map
            .entry("PYTHONUTF8".to_string())
            .or_insert_with(|| "1".to_string());
        env_map
            .entry("PYTHONIOENCODING".to_string())
            .or_insert_with(|| "utf-8".to_string());
        env_map = self.portable_env(env_map);
        self.sanitize_codex_env(&mut env_map);
        codex_env(&self.root_dir, Some(&env_map))
    }

    fn ensure_node_available(
        &mut self,
        env_map: &HashMap<String, String>,
        target: LogTarget,
    ) -> bool {
        if node_executable(&self.root_dir, Some(env_map)).is_some() {
            return true;
        }
        let expected = self.root_dir.join("tools").join("node");
        self.log_issue(
            &format!(
                "Node portable introuvable. Place node dans {} (ex: node.exe) ou ajoute node au PATH.",
                expected.display()
            ),
            "erreur",
            "node",
            target,
        );
        false
    }

    fn tools_env(&self) -> HashMap<String, String> {
        let mut env_map: HashMap<String, String> = std::env::vars().collect();
        env_map
            .entry("PYTHONUTF8".to_string())
            .or_insert_with(|| "1".to_string());
        env_map
            .entry("PYTHONIOENCODING".to_string())
            .or_insert_with(|| "utf-8".to_string());
        env_map = self.portable_env(env_map);
        tools_env(&self.root_dir, Some(&env_map))
    }

    fn wheelhouse_path(&self) -> Option<PathBuf> {
        let wheelhouse = self.root_dir.join("tools").join("wheels");
        if wheelhouse.is_dir() {
            Some(wheelhouse)
        } else {
            None
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        if path.is_dir() {
            return;
        }
        match is_probably_binary(&path, 2048) {
            Ok(true) => {
                self.log_issue(
                    &format!("Binaire/non texte ignore: {}", path.display()),
                    "avertissement",
                    "ouverture_fichier",
                    LogTarget::Main,
                );
                return;
            }
            Err(err) => {
                self.log_issue(
                    &format!("Acces fichier impossible: {} ({err})", path.display()),
                    "erreur",
                    "ouverture_fichier",
                    LogTarget::Main,
                );
                return;
            }
            _ => {}
        }

        let encoding = detect_text_encoding(&path);
        let text = match read_text_with_encoding(&path, &encoding) {
            Ok(text) => text,
            Err(err) => {
                self.log_issue(
                    &format!("Erreur ouverture: {} ({err})", path.display()),
                    "erreur",
                    "ouverture_fichier",
                    LogTarget::Main,
                );
                return;
            }
        };
        self.editor_text = text;
        self.current = Some(OpenFile {
            path,
            encoding,
            dirty: false,
        });
        self.refresh_title();
    }

    fn write_with_encoding(&self, path: &Path, encoding: &str, content: &str) -> Result<bool> {
        let encoding_lower = encoding.to_lowercase();
        if encoding_lower == "utf-8" {
            std::fs::write(path, content.as_bytes()).context("ecriture fichier")?;
            return Ok(false);
        }
        if encoding_lower == "utf-8-sig" {
            let mut data = vec![0xEF, 0xBB, 0xBF];
            data.extend_from_slice(content.as_bytes());
            std::fs::write(path, data).context("ecriture fichier")?;
            return Ok(false);
        }
        if let Some(enc) = encoding_rs::Encoding::for_label(encoding_lower.as_bytes()) {
            let (cow, _, had_errors) = enc.encode(content);
            if had_errors {
                std::fs::write(path, content.as_bytes()).context("ecriture fallback utf-8")?;
                return Ok(true);
            }
            std::fs::write(path, cow.as_ref()).context("ecriture fichier")?;
            return Ok(false);
        }
        std::fs::write(path, content.as_bytes()).context("ecriture fallback utf-8")?;
        Ok(true)
    }
    fn action_save(&mut self) {
        let (path, encoding, dirty) = match self.current.as_ref() {
            Some(current) => (
                current.path.clone(),
                current.encoding.clone(),
                current.dirty,
            ),
            None => {
                self.log_issue(
                    "Aucun fichier ouvert.",
                    "avertissement",
                    "sauvegarde",
                    LogTarget::Main,
                );
                return;
            }
        };
        if !dirty {
            return;
        }

        let content = self.editor_text.clone();
        let result = self.write_with_encoding(&path, &encoding, &content);
        match result {
            Ok(used_utf8_fallback) => {
                if used_utf8_fallback {
                    self.log_issue(
                        &format!("Sauvegarde en UTF-8 (fallback) {}", path.display()),
                        "avertissement",
                        "sauvegarde",
                        LogTarget::Main,
                    );
                } else {
                    self.log_ui(format!("Sauvegarde {}", path.display()));
                }
                if let Some(current) = self.current.as_mut() {
                    if used_utf8_fallback {
                        current.encoding = "utf-8".to_string();
                    }
                    current.dirty = false;
                }
                self.refresh_title();
            }
            Err(err) => {
                self.log_issue(
                    &format!("Erreur sauvegarde: {} ({err})", path.display()),
                    "erreur",
                    "sauvegarde",
                    LogTarget::Main,
                );
            }
        }
    }

    fn action_run(&mut self) {
        let (path, dirty) = match self.current.as_ref() {
            Some(current) => (current.path.clone(), current.dirty),
            None => {
                self.log_issue(
                    "Ouvre un fichier .py.",
                    "avertissement",
                    "execution_python",
                    LogTarget::Main,
                );
                return;
            }
        };
        let is_py = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("py"))
            .unwrap_or(false);
        if !is_py {
            self.log_issue(
                "Ouvre un fichier .py.",
                "avertissement",
                "execution_python",
                LogTarget::Main,
            );
            return;
        }
        if dirty {
            self.action_save();
        }
        let argv = python_run_argv(&path);
        self.log_ui(format!("$ {}", argv.join(" ")));
        let env_map = self.portable_env(std::env::vars().collect());
        self.spawn_process(
            argv,
            env_map,
            "execution python",
            LogTarget::Main,
            ProcessKind::PythonRun,
        );
    }

    fn action_clear_log(&mut self) {
        self.log.clear();
        self.codex_log.clear();
        self.last_codex_message = None;
        self.codex_log_dirty = true;
        self.log_ui("journaux effaces".to_string());
    }

    fn action_reload_tree(&mut self) {
        self.tree = FileTree::new(&self.root_dir);
        self.log_ui("arborescence rechargee".to_string());
    }

    fn action_toggle_codex_view(&mut self) {
        self.codex_compact_view = !self.codex_compact_view;
        self.last_codex_message = None;
        let mode = if self.codex_compact_view {
            "Compact"
        } else {
            "Brut"
        };
        self.codex_log_ui(format!("Mode Codex: {mode}"));
    }

    fn action_toggle_codex_sandbox(&mut self) {
        self.codex_sandbox_mode = Self::next_codex_sandbox_mode(self.codex_sandbox_mode);
        self.codex_log_ui(format!(
            "Sandbox Codex: {}",
            Self::codex_sandbox_label(self.codex_sandbox_mode)
        ));
    }

    fn action_toggle_codex_approval(&mut self) {
        self.codex_approval_policy = Self::next_codex_approval_policy(self.codex_approval_policy);
        self.codex_log_ui(format!(
            "Approbations Codex: {}",
            Self::codex_approval_label(self.codex_approval_policy)
        ));
    }

    fn action_codex_install(&mut self) {
        let _ = self.install_codex(true, LogTarget::Codex);
    }

    fn codex_exec_extra_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.codex_sandbox_supported != Some(false) {
            args.push("--sandbox".to_string());
            args.push(self.codex_sandbox_mode.as_str().to_string());
        }
        if self.codex_approval_supported != Some(false) {
            args.push("--ask-for-approval".to_string());
            args.push(self.codex_approval_policy.as_str().to_string());
        }
        args
    }

    fn codex_sandbox_label(mode: CodexSandboxMode) -> &'static str {
        match mode {
            CodexSandboxMode::ReadOnly => "lecture seule",
            CodexSandboxMode::WorkspaceWrite => "agent (workspace)",
            CodexSandboxMode::DangerFullAccess => "danger (acces complet)",
        }
    }

    fn codex_approval_label(policy: CodexApprovalPolicy) -> &'static str {
        match policy {
            CodexApprovalPolicy::Untrusted => "non fiable",
            CodexApprovalPolicy::OnFailure => "sur echec",
            CodexApprovalPolicy::OnRequest => "sur demande",
            CodexApprovalPolicy::Never => "jamais",
        }
    }

    fn next_codex_sandbox_mode(mode: CodexSandboxMode) -> CodexSandboxMode {
        match mode {
            CodexSandboxMode::ReadOnly => CodexSandboxMode::WorkspaceWrite,
            CodexSandboxMode::WorkspaceWrite => CodexSandboxMode::DangerFullAccess,
            CodexSandboxMode::DangerFullAccess => CodexSandboxMode::ReadOnly,
        }
    }

    fn next_codex_approval_policy(policy: CodexApprovalPolicy) -> CodexApprovalPolicy {
        match policy {
            CodexApprovalPolicy::OnRequest => CodexApprovalPolicy::OnFailure,
            CodexApprovalPolicy::OnFailure => CodexApprovalPolicy::Untrusted,
            CodexApprovalPolicy::Untrusted => CodexApprovalPolicy::Never,
            CodexApprovalPolicy::Never => CodexApprovalPolicy::OnRequest,
        }
    }

    fn approval_flag_error(line: &str) -> bool {
        let lower = line.to_lowercase();
        lower.contains("--ask-for-approval")
            && (lower.contains("unexpected argument")
                || lower.contains("unknown argument")
                || lower.contains("unrecognized"))
    }

    fn sandbox_flag_error(line: &str) -> bool {
        let lower = line.to_lowercase();
        lower.contains("--sandbox")
            && (lower.contains("unexpected argument")
                || lower.contains("unknown argument")
                || lower.contains("unrecognized"))
    }

    fn sandbox_value_error(line: &str) -> bool {
        let lower = line.to_lowercase();
        lower.contains("--sandbox")
            && (lower.contains("invalid value") || lower.contains("possible values"))
    }

    fn handle_approval_flag_line(&mut self, line: &str) -> bool {
        if !self.codex_exec_used_approval_flag || !Self::approval_flag_error(line) {
            return false;
        }
        if self.codex_approval_supported != Some(false) {
            self.codex_approval_supported = Some(false);
            self.codex_log_action(
                "Option --ask-for-approval non supportee par cette version Codex. Relance sans approbations.",
            );
        }
        self.codex_retry_without_approval = true;
        true
    }

    fn handle_sandbox_flag_line(&mut self, line: &str) -> bool {
        if !self.codex_exec_used_sandbox_flag {
            return false;
        }
        if Self::sandbox_flag_error(line) || Self::sandbox_value_error(line) {
            if self.codex_sandbox_supported != Some(false) {
                self.codex_sandbox_supported = Some(false);
                self.codex_log_action(
                    "Option --sandbox non supportee par cette version Codex. Relance sans sandbox (mode par defaut).",
                );
            }
            self.codex_retry_without_sandbox = true;
            return true;
        }
        false
    }

    fn action_codex_login(&mut self) {
        let env_map = self.codex_env();
        if !codex_cli_available(Some(&self.root_dir), Some(&env_map)) {
            if !self.ensure_node_available(&env_map, LogTarget::Codex) {
                return;
            }
            if !self.install_codex(false, LogTarget::Codex) {
                return;
            }
        }
        self.codex_log_ui("Login Codex : navigateur/Device auth selon config.".to_string());
        if !self.codex_device_auth_enabled() {
            self.codex_log_ui(
                "Astuce: si le navigateur ne s'ouvre pas, definis USBIDE_CODEX_DEVICE_AUTH=1 puis relance."
                    .to_string(),
            );
        }
        let argv = codex_login_argv(
            Some(&self.root_dir),
            Some(&env_map),
            self.codex_device_auth_enabled(),
        );
        self.codex_log_ui(format!("$ {}", argv.join(" ")));
        self.spawn_process(
            argv,
            env_map,
            "login Codex",
            LogTarget::Codex,
            ProcessKind::CodexLogin,
        );
    }

    fn action_codex_check(&mut self) {
        let env_map = self.codex_env();
        if !codex_cli_available(Some(&self.root_dir), Some(&env_map)) {
            if !self.ensure_node_available(&env_map, LogTarget::Codex) {
                return;
            }
            self.log_issue(
                "Codex non installe.",
                "avertissement",
                "codex_status",
                LogTarget::Codex,
            );
            return;
        }
        let node_path = node_executable(&self.root_dir, Some(&env_map));
        let entry_path = codex_entrypoint_js(&codex_install_prefix(&self.root_dir));
        let resolved = resolve_in_path("codex", &env_map);
        self.codex_log_ui(format!(
            "node: {}",
            node_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "absent".into())
        ));
        self.codex_log_ui(format!(
            "entrypoint: {}",
            entry_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "absent".into())
        ));
        self.codex_log_ui(format!(
            "codex (PATH): {}",
            resolved
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "absent".into())
        ));
        let argv = codex_status_argv(Some(&self.root_dir), Some(&env_map));
        self.codex_log_ui(format!("$ {}", argv.join(" ")));
        self.spawn_process(
            argv,
            env_map,
            "verification Codex",
            LogTarget::Codex,
            ProcessKind::CodexStatus,
        );
    }

    fn action_dev_tools(&mut self) {
        let raw = std::env::var("USBIDE_DEV_TOOLS")
            .unwrap_or_else(|_| "ruff black mypy pytest".to_string());
        let tools = parse_tool_list(&raw);
        if tools.is_empty() {
            self.log_issue(
                "Liste outils vide.",
                "avertissement",
                "outils_dev",
                LogTarget::Main,
            );
            return;
        }
        let env_map = self.tools_env();
        let prefix = tools_install_prefix(&self.root_dir);
        let _ = std::fs::create_dir_all(&prefix);
        let wheelhouse = self.wheelhouse_path();
        let argv =
            match pip_install_argv(&prefix, &tools, wheelhouse.as_deref(), wheelhouse.is_some()) {
                Ok(argv) => argv,
                Err(err) => {
                    self.log_issue(
                        &format!("Impossible d'installer outils: {err}"),
                        "erreur",
                        "outils_dev",
                        LogTarget::Main,
                    );
                    return;
                }
            };
        self.log_ui(format!("$ {}", argv.join(" ")));
        self.spawn_process(
            argv,
            env_map,
            "installation outils dev",
            LogTarget::Main,
            ProcessKind::DevTools,
        );
    }

    fn action_build_exe(&mut self) {
        let (path, dirty) = match self.current.as_ref() {
            Some(current) => (current.path.clone(), current.dirty),
            None => {
                self.log_issue(
                    "Ouvre un fichier .py.",
                    "avertissement",
                    "build_exe",
                    LogTarget::Main,
                );
                return;
            }
        };
        let is_py = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("py"))
            .unwrap_or(false);
        if !is_py {
            self.log_issue(
                "Ouvre un fichier .py.",
                "avertissement",
                "build_exe",
                LogTarget::Main,
            );
            return;
        }
        if dirty {
            self.action_save();
        }
        let env_map = self.tools_env();
        if !pyinstaller_available(Some(&self.root_dir), Some(&env_map)) {
            if !self.install_pyinstaller(false) {
                self.log_issue(
                    "PyInstaller indisponible.",
                    "erreur",
                    "build_exe",
                    LogTarget::Main,
                );
                return;
            }
        }
        let dist_dir = self.root_dir.join("dist");
        let _ = std::fs::create_dir_all(&dist_dir);
        let argv = match pyinstaller_build_argv(
            &path,
            &dist_dir,
            false,
            Some(&self.root_dir.join("tmp")),
            None,
        ) {
            Ok(argv) => argv,
            Err(err) => {
                self.log_issue(
                    &format!("Erreur build: {err}"),
                    "erreur",
                    "build_exe",
                    LogTarget::Main,
                );
                return;
            }
        };
        self.log_ui(format!("$ {}", argv.join(" ")));
        self.spawn_process(
            argv,
            env_map,
            "construction exe",
            LogTarget::Main,
            ProcessKind::PyInstallerBuild,
        );
    }
    fn install_pyinstaller(&mut self, force: bool) -> bool {
        let env_map = self.tools_env();
        if !force && pyinstaller_available(Some(&self.root_dir), Some(&env_map)) {
            return true;
        }
        if !force && self.pyinstaller_install_attempted {
            return false;
        }
        self.pyinstaller_install_attempted = true;
        let prefix = tools_install_prefix(&self.root_dir);
        let _ = std::fs::create_dir_all(&prefix);
        let wheelhouse = self.wheelhouse_path();
        let argv =
            match pyinstaller_install_argv(&prefix, wheelhouse.as_deref(), wheelhouse.is_some()) {
                Ok(argv) => argv,
                Err(err) => {
                    self.log_issue(
                        &format!("Impossible d'installer PyInstaller: {err}"),
                        "erreur",
                        "installation_pyinstaller",
                        LogTarget::Main,
                    );
                    return false;
                }
            };
        self.log_ui(format!(
            "Installation PyInstaller (bin={})",
            prefix.display()
        ));
        self.log_ui(format!("$ {}", argv.join(" ")));
        self.spawn_process(
            argv,
            env_map,
            "installation PyInstaller",
            LogTarget::Main,
            ProcessKind::PyInstallerInstall,
        );
        true
    }

    fn codex_device_auth_enabled(&self) -> bool {
        std::env::var("USBIDE_CODEX_DEVICE_AUTH")
            .map(|v| {
                matches!(
                    v.trim().to_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    fn codex_auto_install_enabled(&self) -> bool {
        std::env::var("USBIDE_CODEX_AUTO_INSTALL")
            .map(|v| {
                !matches!(
                    v.trim().to_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                )
            })
            .unwrap_or(true)
    }

    fn install_codex(&mut self, force: bool, target: LogTarget) -> bool {
        let env_map = self.codex_env();
        if !force && codex_cli_available(Some(&self.root_dir), Some(&env_map)) {
            return true;
        }
        if !force && self.codex_install_attempted {
            self.log_issue(
                "Installation Codex deja tentee. (bouton Installer pour forcer)",
                "avertissement",
                "installation_codex",
                target,
            );
            return false;
        }
        if !force && !self.codex_auto_install_enabled() {
            self.log_issue(
                "Auto-install Codex desactive. (bouton Installer)",
                "avertissement",
                "installation_codex",
                target,
            );
            return false;
        }
        if !self.ensure_node_available(&env_map, target) {
            return false;
        }
        self.codex_install_attempted = true;
        let package = std::env::var("USBIDE_CODEX_NPM_PACKAGE")
            .unwrap_or_else(|_| "@openai/codex".to_string());
        let prefix = codex_install_prefix(&self.root_dir);
        if let Err(err) = std::fs::create_dir_all(&prefix) {
            self.log_issue(
                &format!(
                    "Impossible de creer le dossier d'installation Codex: {} ({err})",
                    prefix.display()
                ),
                "erreur",
                "installation_codex",
                target,
            );
            return false;
        }
        let argv = match codex_install_argv(&self.root_dir, &prefix, &package) {
            Ok(argv) => argv,
            Err(CodexError::NodeMissing) => {
                self.log_issue(
                    "Node portable introuvable. Place node dans tools/node (ex: node.exe) ou ajoute node au PATH.",
                    "erreur",
                    "installation_codex",
                    target,
                );
                return false;
            }
            Err(CodexError::NpmMissing) => {
                self.log_issue(
                    "npm-cli.js introuvable. Verifie ton Node portable (npm inclus).",
                    "erreur",
                    "installation_codex",
                    target,
                );
                return false;
            }
            Err(err) => {
                self.log_issue(
                    &format!("Impossible d'installer Codex: {err}"),
                    "erreur",
                    "installation_codex",
                    target,
                );
                return false;
            }
        };
        self.push_log(
            target,
            format!(
                "Installation Codex package={package} prefix={}",
                prefix.display()
            ),
            LogKind::Info,
        );
        self.push_log(target, format!("$ {}", argv.join(" ")), LogKind::Info);
        self.spawn_process(
            argv,
            env_map,
            "installation Codex",
            target,
            ProcessKind::CodexInstall,
        );
        true
    }

    fn run_shell(&mut self, cmd: String) {
        if cmd.is_empty() {
            return;
        }
        self.log_ui(format!("$ {cmd}"));
        let argv = if cfg!(windows) {
            windows_cmd_argv(&cmd)
        } else {
            vec!["sh".to_string(), "-lc".to_string(), cmd]
        };
        let env_map = self.portable_env(std::env::vars().collect());
        self.spawn_process(
            argv,
            env_map,
            "commande shell",
            LogTarget::Main,
            ProcessKind::Shell,
        );
    }

    fn run_codex(&mut self, prompt: String) {
        if prompt.is_empty() {
            return;
        }
        if self.codex_compact_view {
            self.codex_log_user_message(&prompt);
        }
        let env_map = self.codex_env();
        if !codex_cli_available(Some(&self.root_dir), Some(&env_map)) {
            if !self.ensure_node_available(&env_map, LogTarget::Codex) {
                return;
            }
            if self.install_codex(false, LogTarget::Codex) {
                self.pending_codex_prompt = Some(prompt);
            }
            return;
        }

        self.pending_codex_prompt = Some(prompt);
        let argv = codex_status_argv(Some(&self.root_dir), Some(&env_map));
        self.spawn_process(
            argv,
            env_map,
            "codex_status",
            LogTarget::Codex,
            ProcessKind::CodexStatus,
        );
    }

    fn spawn_process(
        &mut self,
        argv: Vec<String>,
        env_map: HashMap<String, String>,
        contexte: &str,
        target: LogTarget,
        kind: ProcessKind,
    ) {
        match stream_subprocess(&argv, Some(&self.root_dir), Some(&env_map)) {
            Ok(handle) => {
                self.running.push(RunningProcess {
                    handle,
                    kind,
                    target,
                    contexte: contexte.to_string(),
                });
            }
            Err(err) => {
                self.log_issue(
                    &format!("Erreur execution {contexte}: {err}"),
                    "erreur",
                    contexte,
                    target,
                );
            }
        }
    }

    fn drain_process_events(&mut self) {
        let mut active = std::mem::take(&mut self.running);
        let mut remaining = Vec::new();

        for mut proc in active.drain(..) {
            let mut finished = false;
            while let Ok(event) = proc.handle.rx.try_recv() {
                match event.kind {
                    ProcEventKind::Line => {
                        self.handle_process_line(&mut proc, &event.text);
                    }
                    ProcEventKind::Exit => {
                        if let Some(code) = event.returncode {
                            if code != 0 {
                                self.log_issue(
                                    &format!("{} terminee en erreur (rc={code}).", proc.contexte),
                                    "erreur",
                                    &proc.contexte,
                                    proc.target,
                                );
                            }
                        }
                        self.handle_process_exit(&mut proc, event.returncode);
                        finished = true;
                        break;
                    }
                }
            }

            if finished {
                proc.handle.join();
            } else {
                remaining.push(proc);
            }
        }

        let mut spawned = std::mem::take(&mut self.running);
        remaining.append(&mut spawned);
        self.running = remaining;
    }
    fn handle_process_line(&mut self, proc: &mut RunningProcess, line: &str) {
        match proc.kind {
            ProcessKind::CodexExec => self.handle_codex_line(line),
            _ => self.push_log(proc.target, line.to_string(), LogKind::Info),
        }
    }

    fn handle_process_exit(&mut self, proc: &mut RunningProcess, code: Option<i32>) {
        match proc.kind {
            ProcessKind::CodexStatus => {
                if let Some(prompt) = self.pending_codex_prompt.take() {
                    if code == Some(0) {
                        let env_map = self.codex_env();
                        let extra_args = self.codex_exec_extra_args();
                        self.codex_exec_used_sandbox_flag =
                            extra_args.iter().any(|arg| arg == "--sandbox");
                        self.codex_exec_used_approval_flag =
                            extra_args.iter().any(|arg| arg == "--ask-for-approval");
                        self.codex_last_prompt = Some(prompt.clone());
                        match codex_exec_argv(
                            &prompt,
                            Some(&self.root_dir),
                            Some(&env_map),
                            true,
                            Some(&extra_args),
                        ) {
                            Ok(argv) => {
                                if !self.codex_compact_view {
                                    self.codex_log_ui(format!("$ {}", argv.join(" ")));
                                }
                                self.spawn_process(
                                    argv,
                                    env_map,
                                    "codex_exec",
                                    LogTarget::Codex,
                                    ProcessKind::CodexExec,
                                );
                            }
                            Err(err) => {
                                self.log_issue(
                                    &format!("Erreur Codex: {err}"),
                                    "erreur",
                                    "codex_exec",
                                    LogTarget::Codex,
                                );
                            }
                        }
                    } else {
                        self.codex_log_action(
                            "Echec de la verification du login Codex (status en erreur).",
                        );
                        self.codex_log_action(
                            "Si tu n'es pas authentifie, fais Login puis recommence.",
                        );
                        self.codex_log_action(
                            "Si tu es deja authentifie, verifie l'installation et la connexion.",
                        );
                        if !self.codex_device_auth_enabled() {
                            self.codex_log_action(
                                "Astuce: si le navigateur ne s'ouvre pas, definis USBIDE_CODEX_DEVICE_AUTH=1.",
                            );
                        }
                    }
                }
            }
            ProcessKind::CodexExec => {
                if self.codex_compact_view && !self.codex_assistant_buffer.is_empty() {
                    let message = std::mem::take(&mut self.codex_assistant_buffer);
                    self.codex_log_message(&message);
                }
                if self.codex_retry_without_sandbox || self.codex_retry_without_approval {
                    self.codex_retry_without_sandbox = false;
                    self.codex_retry_without_approval = false;
                    if let Some(prompt) = self.codex_last_prompt.clone() {
                        let env_map = self.codex_env();
                        let extra_args = self.codex_exec_extra_args();
                        self.codex_exec_used_sandbox_flag =
                            extra_args.iter().any(|arg| arg == "--sandbox");
                        self.codex_exec_used_approval_flag =
                            extra_args.iter().any(|arg| arg == "--ask-for-approval");
                        if let Ok(argv) = codex_exec_argv(
                            &prompt,
                            Some(&self.root_dir),
                            Some(&env_map),
                            true,
                            Some(&extra_args),
                        ) {
                            self.codex_log_ui(format!("$ {}", argv.join(" ")));
                            self.spawn_process(
                                argv,
                                env_map,
                                "codex_exec",
                                LogTarget::Codex,
                                ProcessKind::CodexExec,
                            );
                        }
                    }
                }
            }
            ProcessKind::CodexInstall => {
                let env_map = self.codex_env();
                if codex_cli_available(Some(&self.root_dir), Some(&env_map)) {
                    self.codex_log_ui("Codex installe.".to_string());
                    if let Some(prompt) = self.pending_codex_prompt.take() {
                        self.run_codex(prompt);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_codex_line(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.handle_sandbox_flag_line(trimmed) || self.handle_approval_flag_line(trimmed) {
            return;
        }
        if self.codex_retry_without_sandbox || self.codex_retry_without_approval {
            return;
        }
        if let Some(translated) = translate_codex_line(trimmed) {
            if self.codex_compact_view {
                self.codex_log_action(&translated);
            } else {
                self.codex_log_ui(translated);
            }
            return;
        }

        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(val) => val,
            Err(_) => {
                if self.codex_compact_view {
                    self.codex_log_action(trimmed);
                } else {
                    self.codex_log_ui(trimmed.to_string());
                }
                return;
            }
        };

        let event_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if self.codex_compact_view {
            if matches!(
                event_type,
                "response.output_text.delta" | "response.output_text"
            ) {
                let delta = value
                    .get("delta")
                    .or_else(|| value.get("text"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                if !delta.is_empty() {
                    self.codex_assistant_buffer.push_str(delta);
                }
                return;
            }
            if matches!(
                event_type,
                "response.output_text.done" | "response.output_item.done" | "response.completed"
            ) {
                if !self.codex_assistant_buffer.is_empty() {
                    let message = std::mem::take(&mut self.codex_assistant_buffer);
                    self.codex_log_message(&message);
                }
                return;
            }
        }

        if event_type == "error" {
            let msg = value
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if self.codex_compact_view {
                if let Some(translated) = translate_codex_line(msg) {
                    self.codex_log_action(&translated);
                } else if let Some(status) = extract_status_code(msg) {
                    self.codex_log_action(&format!("Erreur Codex HTTP {status}."));
                    if let Some(hint) = codex_hint_for_status(status) {
                        self.codex_log_action(&hint);
                    }
                } else {
                    self.codex_log_action(
                        "Erreur Codex: une erreur est survenue. Consulte le journal ou relance.",
                    );
                }
            } else {
                self.codex_log_ui("Erreur Codex: une erreur est survenue.".to_string());
            }
            return;
        }

        if event_type == "turn.failed" {
            let msg = value
                .get("error")
                .and_then(|err| err.get("message").or_else(|| err.get("text")))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if self.codex_compact_view {
                if let Some(translated) = translate_codex_line(msg) {
                    self.codex_log_action(&translated);
                } else if let Some(status) = extract_status_code(msg) {
                    self.codex_log_action(&format!("Tache echouee HTTP {status}."));
                    if let Some(hint) = codex_hint_for_status(status) {
                        self.codex_log_action(&hint);
                    }
                } else {
                    self.codex_log_action("Tache echouee: une erreur est survenue.");
                }
            } else {
                self.codex_log_ui("Tache echouee.".to_string());
            }
            return;
        }

        if self.codex_compact_view {
            for item in extract_display_items(&value) {
                match item.kind {
                    DisplayKind::Assistant => self.codex_log_message(&item.message),
                    DisplayKind::User => self.codex_log_user_message(&item.message),
                    DisplayKind::Action => self.codex_log_action(&item.message),
                }
            }
        } else if let Some(event_type) = value.get("type").and_then(serde_json::Value::as_str) {
            self.codex_log_ui(format!("[{event_type}] {value}"));
        } else {
            self.codex_log_ui(value.to_string());
        }
    }

    fn codex_log_entry(&mut self, msg: &str, label: &str, kind: LogKind) {
        let cleaned = msg.trim();
        if cleaned.is_empty() {
            return;
        }
        let fingerprint = format!("{label}:{cleaned}");
        if self.last_codex_message.as_deref() == Some(&fingerprint) {
            return;
        }
        self.last_codex_message = Some(fingerprint);
        self.push_log(LogTarget::Codex, label.to_string(), LogKind::Action);
        self.push_log(LogTarget::Codex, cleaned.to_string(), kind);
        self.push_log(LogTarget::Codex, String::new(), LogKind::Info);
    }

    fn codex_log_user_message(&mut self, msg: &str) {
        self.codex_log_entry(msg, "Utilisateur", LogKind::User);
    }

    fn codex_log_action(&mut self, msg: &str) {
        self.codex_log_entry(msg, "Action", LogKind::Action);
    }

    fn codex_log_message(&mut self, msg: &str) {
        self.codex_log_entry(msg, "Assistant", LogKind::Assistant);
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_process_events();
        self.handle_shortcuts(ctx);
        self.update_window_title(ctx);

        egui::TopBottomPanel::top("header")
            .resizable(false)
            .show(ctx, |ui| self.draw_header(ui));

        egui::SidePanel::left("files")
            .resizable(true)
            .default_width(280.0)
            .min_width(220.0)
            .max_width(420.0)
            .show(ctx, |ui| self.draw_file_tree(ui));

        egui::TopBottomPanel::bottom("bottom")
            .resizable(true)
            .default_height({
                let h = ctx.input(|i| i.screen_rect().height());
                (h * 0.30).clamp(240.0, 360.0)
            })
            .min_height(220.0)
            .max_height({
                let h = ctx.input(|i| i.screen_rect().height());
                (h * 0.45).clamp(280.0, 480.0)
            })
            .show(ctx, |ui| {
                let height = ui.available_height();
                ui.columns(2, |columns| {
                    columns[0].set_min_height(height);
                    columns[1].set_min_height(height);
                    self.draw_command_panel(&mut columns[0]);
                    self.draw_codex_panel(&mut columns[1]);
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_editor(ui);
        });

        ctx.request_repaint_after(Duration::from_millis(33));
    }
}
