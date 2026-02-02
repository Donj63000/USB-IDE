use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use tui_textarea::{Input, TextArea};

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

const LOG_LIMIT: usize = 2000;
const APP_NAME: &str = "ValDev Pro v1";

#[derive(Debug, Clone)]
struct OpenFile {
    path: PathBuf,
    encoding: String,
    dirty: bool,
}

#[derive(Debug, Clone)]
struct LogLine {
    text: String,
    style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tree,
    Editor,
    Cmd,
    Codex,
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

struct InputField {
    value: String,
    cursor: usize,
}

impl InputField {
    fn new() -> Self {
        Self {
            value: String::new(),
            cursor: 0,
        }
    }

    fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    fn insert_char(&mut self, ch: char) {
        let mut chars: Vec<char> = self.value.chars().collect();
        if self.cursor <= chars.len() {
            chars.insert(self.cursor, ch);
            self.cursor += 1;
        }
        self.value = chars.into_iter().collect();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.value.chars().collect();
        if self.cursor <= chars.len() {
            chars.remove(self.cursor - 1);
            self.cursor -= 1;
            self.value = chars.into_iter().collect();
        }
    }

    fn delete(&mut self) {
        let mut chars: Vec<char> = self.value.chars().collect();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.value = chars.into_iter().collect();
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        let len = self.value.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match key.code {
            KeyCode::Enter => {
                let submitted = self.value.trim().to_string();
                self.clear();
                if submitted.is_empty() {
                    None
                } else {
                    Some(submitted)
                }
            }
            KeyCode::Backspace => {
                self.backspace();
                None
            }
            KeyCode::Delete => {
                self.delete();
                None
            }
            KeyCode::Left => {
                self.move_left();
                None
            }
            KeyCode::Right => {
                self.move_right();
                None
            }
            KeyCode::Home => {
                self.cursor = 0;
                None
            }
            KeyCode::End => {
                self.cursor = self.value.chars().count();
                None
            }
            KeyCode::Char(ch) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.insert_char(ch);
                }
                None
            }
            _ => None,
        }
    }
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
    state: ListState,
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
            state: ListState::default(),
        };
        tree.rebuild_visible();
        tree.state.select(Some(0));
        tree
    }

    fn rebuild_visible(&mut self) {
        self.visible.clear();
        let mut entries = Vec::new();
        flatten_tree(&self.root, 0, &self.expanded, &mut entries);
        self.visible = entries;
        if self.visible.is_empty() {
            self.state.select(None);
        } else if self.state.selected().unwrap_or(0) >= self.visible.len() {
            self.state.select(Some(self.visible.len() - 1));
        }
    }

    fn selected_entry(&self) -> Option<&TreeEntry> {
        self.state.selected().and_then(|idx| self.visible.get(idx))
    }

    fn select_next(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let next = match self.state.selected() {
            Some(idx) => (idx + 1).min(self.visible.len() - 1),
            None => 0,
        };
        self.state.select(Some(next));
    }

    fn select_prev(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let prev = match self.state.selected() {
            Some(idx) => idx.saturating_sub(1),
            None => 0,
        };
        self.state.select(Some(prev));
    }

    fn toggle_dir(&mut self) {
        let path = match self.selected_entry() {
            Some(entry) if entry.is_dir => entry.path.clone(),
            _ => return,
        };
        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
        self.rebuild_visible();
    }
}

fn build_tree(path: &Path) -> FileNode {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(".")
        .to_string();
    let is_dir = path.is_dir();
    let mut children = Vec::new();
    if is_dir {
        if let Ok(read_dir) = fs::read_dir(path) {
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
    let mut stdout = std::io::stdout();
    enable_raw_mode().context("impossible d'activer le mode raw")?;
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(root_dir)?;
    let res = app.run(&mut terminal);
    disable_raw_mode().ok();
    let mut stdout = std::io::stdout();
    stdout.execute(LeaveAlternateScreen).ok();
    res
}

struct App {
    root_dir: PathBuf,
    current: Option<OpenFile>,
    tree: FileTree,
    editor: TextArea<'static>,
    cmd_input: InputField,
    codex_input: InputField,
    log: Vec<LogLine>,
    codex_log: Vec<LogLine>,
    focus: Focus,
    title: String,
    sub_title: String,
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
    last_codex_message: Option<String>,
    codex_assistant_buffer: String,
    running: Vec<RunningProcess>,
    bug_log_path: PathBuf,
    codex_install_attempted: bool,
    pyinstaller_install_attempted: bool,
    last_codex_width: u16,
    pending_codex_prompt: Option<String>,
}

impl App {
    fn new(root_dir: PathBuf) -> Result<Self> {
        let root_dir = root_dir.canonicalize().unwrap_or(root_dir);
        let bug_log_path = root_dir.join("bug.md");
        let tree = FileTree::new(&root_dir);
        let mut app = Self {
            root_dir,
            current: None,
            tree,
            editor: Self::make_editor(),
            cmd_input: InputField::new(),
            codex_input: InputField::new(),
            log: Vec::new(),
            codex_log: Vec::new(),
            focus: Focus::Tree,
            title: APP_NAME.to_string(),
            sub_title: String::new(),
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
            last_codex_message: None,
            codex_assistant_buffer: String::new(),
            running: Vec::new(),
            bug_log_path,
            codex_install_attempted: false,
            pyinstaller_install_attempted: false,
            last_codex_width: 80,
            pending_codex_prompt: None,
        };
        app.ensure_portable_dirs();
        app.refresh_title();
        app.log_ui(format!(
            "{APP_NAME}\nRoot: {}\nShell: champ 'Commande' - Codex: champ 'Codex' - Ctrl+K login - Ctrl+I install - Ctrl+O sandbox - Ctrl+P approb\n",
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
        Ok(app)
    }

    fn make_editor() -> TextArea<'static> {
        let mut editor = TextArea::default();
        editor.set_block(Block::default().borders(Borders::ALL).title("Editeur"));
        editor
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        let tick_rate = Duration::from_millis(50);
        let mut last_tick = Instant::now();
        loop {
            terminal.draw(|f| self.draw(f))?;
            self.drain_process_events();

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key) {
                        break;
                    }
                }
            }
            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }
        }
        Ok(())
    }

    fn draw(&mut self, f: &mut ratatui::Frame<'_>) {
        let area = f.area();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        self.draw_header(f, layout[0]);
        self.draw_body(f, layout[1]);
        self.draw_footer(f, layout[2]);
    }

    fn draw_header(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let title = Line::from(vec![
            Span::styled(&self.title, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(&self.sub_title, Style::default().fg(Color::Gray)),
        ]);
        let header = Paragraph::new(Text::from(title));
        f.render_widget(header, area);
    }

    fn draw_footer(&self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let help = "Ctrl+S sauver | F5 executer | Ctrl+O sandbox | Ctrl+P approb | Ctrl+Q quitter | Tab focus";
        let footer = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
        f.render_widget(footer, area);
    }

    fn draw_body(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        self.draw_tree(f, chunks[0]);
        self.draw_right(f, chunks[1]);
    }

    fn draw_tree(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let mut items = Vec::new();
        for entry in &self.tree.visible {
            let indent = "  ".repeat(entry.depth);
            let is_expanded = self.tree.expanded.contains(&entry.path);
            let icon = if entry.is_dir {
                if is_expanded { "-" } else { "+" }
            } else {
                " "
            };
            let text = format!("{indent}{icon} {}", entry.name);
            items.push(ListItem::new(Line::from(text)));
        }
        let block = Self::block_with_focus("Fichiers", self.focus == Focus::Tree);
        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue));
        f.render_stateful_widget(list, area, &mut self.tree.state);
    }

    fn draw_right(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        self.draw_editor(f, chunks[0]);
        self.draw_bottom(f, chunks[1]);
    }

    fn draw_editor(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let block = Self::block_with_focus("Editeur", self.focus == Focus::Editor);
        self.editor.set_block(block);
        f.render_widget(self.editor.widget(), area);
        if self.focus == Focus::Editor {
            let (row, col) = self.editor.cursor();
            let x = area.x + col as u16 + 1;
            let y = area.y + row as u16 + 1;
            f.set_cursor_position((x, y));
        }
    }

    fn draw_bottom(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.draw_shell(f, chunks[0]);
        self.draw_codex(f, chunks[1]);
    }

    fn draw_shell(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let input_block = Self::block_with_focus("Commande", self.focus == Focus::Cmd);
        let input = Paragraph::new(self.cmd_input.value.as_str()).block(input_block);
        f.render_widget(input, chunks[0]);
        if self.focus == Focus::Cmd {
            let cursor_x = chunks[0].x + 1 + self.cmd_input.cursor as u16;
            let cursor_y = chunks[0].y + 1;
            f.set_cursor_position((cursor_x, cursor_y));
        }

        let log_block = Block::default().borders(Borders::ALL).title("Journal");
        let log_text = self.render_log(&self.log, chunks[1].height.saturating_sub(2) as usize);
        let log = Paragraph::new(log_text)
            .block(log_block)
            .wrap(Wrap { trim: false });
        f.render_widget(log, chunks[1]);
    }

    fn draw_codex(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let input_block = Self::block_with_focus("Codex", self.focus == Focus::Codex);
        let input = Paragraph::new(self.codex_input.value.as_str()).block(input_block);
        f.render_widget(input, chunks[0]);
        if self.focus == Focus::Codex {
            let cursor_x = chunks[0].x + 1 + self.codex_input.cursor as u16;
            let cursor_y = chunks[0].y + 1;
            f.set_cursor_position((cursor_x, cursor_y));
        }

        let log_block = Block::default().borders(Borders::ALL).title("Sortie Codex");
        self.last_codex_width = chunks[1].width;
        let log_text =
            self.render_log(&self.codex_log, chunks[1].height.saturating_sub(2) as usize);
        let log = Paragraph::new(log_text)
            .block(log_block)
            .wrap(Wrap { trim: false });
        f.render_widget(log, chunks[1]);
    }

    fn render_log(&self, log: &[LogLine], max_lines: usize) -> Text<'_> {
        let start = log.len().saturating_sub(max_lines);
        let lines: Vec<Line> = log[start..]
            .iter()
            .map(|entry| Line::from(Span::styled(entry.text.clone(), entry.style)))
            .collect();
        Text::from(lines)
    }

    fn block_with_focus<'a>(title: &'a str, focused: bool) -> Block<'a> {
        let style = if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(style)
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.handle_global_shortcut(key) {
            return true;
        }

        match self.focus {
            Focus::Tree => self.handle_tree_key(key),
            Focus::Editor => self.handle_editor_key(key),
            Focus::Cmd => self.handle_cmd_key(key),
            Focus::Codex => self.handle_codex_key(key),
        }

        false
    }

    fn handle_global_shortcut(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('q') => return true,
                KeyCode::Char('s') => {
                    self.action_save();
                    return false;
                }
                KeyCode::Char('l') => {
                    self.action_clear_log();
                    return false;
                }
                KeyCode::Char('r') => {
                    self.action_reload_tree();
                    return false;
                }
                KeyCode::Char('k') => {
                    self.action_codex_login();
                    return false;
                }
                KeyCode::Char('t') => {
                    self.action_codex_check();
                    return false;
                }
                KeyCode::Char('i') => {
                    self.action_codex_install();
                    return false;
                }
                KeyCode::Char('m') => {
                    self.action_toggle_codex_view();
                    return false;
                }
                KeyCode::Char('o') => {
                    self.action_toggle_codex_sandbox();
                    return false;
                }
                KeyCode::Char('p') => {
                    self.action_toggle_codex_approval();
                    return false;
                }
                KeyCode::Char('e') => {
                    self.action_build_exe();
                    return false;
                }
                KeyCode::Char('d') => {
                    self.action_dev_tools();
                    return false;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::F(5) => {
                self.action_run();
                false
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Tree => Focus::Editor,
                    Focus::Editor => Focus::Cmd,
                    Focus::Cmd => Focus::Codex,
                    Focus::Codex => Focus::Tree,
                };
                false
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Tree => Focus::Codex,
                    Focus::Editor => Focus::Tree,
                    Focus::Cmd => Focus::Editor,
                    Focus::Codex => Focus::Cmd,
                };
                false
            }
            _ => false,
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.tree.select_prev(),
            KeyCode::Down => self.tree.select_next(),
            KeyCode::Right | KeyCode::Enter => {
                if let Some(entry) = self.tree.selected_entry() {
                    if entry.is_dir {
                        self.tree.toggle_dir();
                    } else {
                        self.open_file(entry.path.clone());
                    }
                }
            }
            KeyCode::Left => self.tree.toggle_dir(),
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let mut changed = false;
        if matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter | KeyCode::Tab
        ) {
            changed = true;
        }
        let input = Input::from(key);
        self.editor.input(input);
        if changed {
            if let Some(current) = self.current.as_mut() {
                current.dirty = true;
                self.refresh_title();
            }
        }
    }

    fn handle_cmd_key(&mut self, key: KeyEvent) {
        if let Some(cmd) = self.cmd_input.handle_key(key) {
            self.run_shell(cmd);
        }
    }

    fn handle_codex_key(&mut self, key: KeyEvent) {
        if let Some(prompt) = self.codex_input.handle_key(key) {
            self.run_codex(prompt);
        }
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

    fn log_ui(&mut self, msg: String) {
        self.push_log(LogTarget::Main, msg, Style::default());
    }

    fn codex_log_ui(&mut self, msg: String) {
        self.push_log(LogTarget::Codex, msg, Style::default());
    }

    fn codex_log_output(&mut self, msg: String) {
        self.push_log(LogTarget::Codex, msg, Style::default());
    }

    fn push_log(&mut self, target: LogTarget, msg: String, style: Style) {
        let lines: Vec<String> = msg.split('\n').map(|s| s.to_string()).collect();
        let store = match target {
            LogTarget::Main => &mut self.log,
            LogTarget::Codex => &mut self.codex_log,
        };
        for line in lines {
            store.push(LogLine { text: line, style });
        }
        if store.len() > LOG_LIMIT {
            let drain = store.len() - LOG_LIMIT;
            store.drain(0..drain);
        }
    }

    fn log_issue(&mut self, msg: &str, niveau: &str, contexte: &str, target: LogTarget) {
        let styled = Style::default().fg(Color::Red);
        self.push_log(target, msg.to_string(), styled);
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
        let _ = fs::OpenOptions::new()
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
            let _ = fs::create_dir_all(path);
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

        let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let mut editor = TextArea::from(lines);
        editor.set_block(Block::default().borders(Borders::ALL).title("Editeur"));
        self.editor = editor;
        self.current = Some(OpenFile {
            path,
            encoding,
            dirty: false,
        });
        self.refresh_title();
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

        let content = self.editor.lines().join("\n");
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

    fn write_with_encoding(&self, path: &Path, encoding: &str, content: &str) -> Result<bool> {
        let encoding_lower = encoding.to_lowercase();
        if encoding_lower == "utf-8" {
            fs::write(path, content.as_bytes()).context("ecriture fichier")?;
            return Ok(false);
        }
        if encoding_lower == "utf-8-sig" {
            let mut data = vec![0xEF, 0xBB, 0xBF];
            data.extend_from_slice(content.as_bytes());
            fs::write(path, data).context("ecriture fichier")?;
            return Ok(false);
        }
        if let Some(enc) = encoding_rs::Encoding::for_label(encoding_lower.as_bytes()) {
            let (cow, _, had_errors) = enc.encode(content);
            if had_errors {
                fs::write(path, content.as_bytes()).context("ecriture fallback utf-8")?;
                return Ok(true);
            }
            fs::write(path, cow.as_ref()).context("ecriture fichier")?;
            return Ok(false);
        }
        fs::write(path, content.as_bytes()).context("ecriture fallback utf-8")?;
        Ok(true)
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
                "Astuce: si le navigateur ne s'ouvre pas, definis USBIDE_CODEX_DEVICE_AUTH=1 puis relance Ctrl+K."
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
                .unwrap_or("absent".into())
        ));
        self.codex_log_ui(format!(
            "entrypoint: {}",
            entry_path
                .map(|p| p.display().to_string())
                .unwrap_or("absent".into())
        ));
        self.codex_log_ui(format!(
            "codex (PATH): {}",
            resolved
                .map(|p| p.display().to_string())
                .unwrap_or("absent".into())
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
        let _ = fs::create_dir_all(&prefix);
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
        let _ = fs::create_dir_all(&dist_dir);
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
        let _ = fs::create_dir_all(&prefix);
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
                "Installation Codex deja tentee. (Ctrl+I pour forcer)",
                "avertissement",
                "installation_codex",
                target,
            );
            return false;
        }
        if !force && !self.codex_auto_install_enabled() {
            self.log_issue(
                "Auto-install Codex desactive. (Ctrl+I pour installer)",
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
        if let Err(err) = fs::create_dir_all(&prefix) {
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
            Style::default(),
        );
        self.push_log(target, format!("$ {}", argv.join(" ")), Style::default());
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
            _ => self.push_log(proc.target, line.to_string(), Style::default()),
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
                                "Astuce: si le navigateur ne s'ouvre pas, definis USBIDE_CODEX_DEVICE_AUTH=1 puis Ctrl+K.",
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
                    self.codex_log_output(trimmed.to_string());
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
            self.codex_log_output(format!("[{event_type}] {value}"));
        } else {
            self.codex_log_output(value.to_string());
        }
    }

    fn codex_log_entry(&mut self, msg: &str, label: &str, kind: &str) {
        let cleaned = msg.trim();
        if cleaned.is_empty() {
            return;
        }
        let fingerprint = format!("{kind}:{cleaned}");
        if self.last_codex_message.as_deref() == Some(&fingerprint) {
            return;
        }
        self.last_codex_message = Some(fingerprint);
        let (label_style, line_style) = match kind {
            "assistant" => (
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Green),
            ),
            "user" => (
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Blue),
            ),
            "action" => (
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::DarkGray),
            ),
            _ => (
                Style::default().add_modifier(Modifier::BOLD),
                Style::default(),
            ),
        };
        self.push_log(LogTarget::Codex, label.to_string(), label_style);
        let width = self.last_codex_width.saturating_sub(4) as usize;
        for line in crate::codex::wrap_text(msg, width) {
            if line.is_empty() {
                self.push_log(LogTarget::Codex, String::new(), Style::default());
            } else {
                self.push_log(LogTarget::Codex, line, line_style);
            }
        }
        self.push_log(LogTarget::Codex, String::new(), Style::default());
    }

    fn codex_log_user_message(&mut self, msg: &str) {
        self.codex_log_entry(msg, "Utilisateur", "user");
    }

    fn codex_log_action(&mut self, msg: &str) {
        self.codex_log_entry(msg, "Action", "action");
    }

    fn codex_log_message(&mut self, msg: &str) {
        self.codex_log_entry(msg, "Assistant", "assistant");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_lock<F: FnOnce()>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        f();
    }

    fn canonical_root(path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
    fn refresh_title_sans_fichier() {
        let dir = TempDir::new().unwrap();
        let app = App::new(dir.path().to_path_buf()).unwrap();
        let expected_root = canonical_root(dir.path());
        assert_eq!(app.title, APP_NAME);
        assert_eq!(app.sub_title, expected_root.display().to_string());
    }

    #[test]
    fn refresh_title_avec_fichier_dirty() {
        let dir = TempDir::new().unwrap();
        let mut app = App::new(dir.path().to_path_buf()).unwrap();
        app.current = Some(OpenFile {
            path: dir.path().join("main.py"),
            encoding: "utf-8".to_string(),
            dirty: true,
        });
        app.refresh_title();
        assert_eq!(app.title, format!("{APP_NAME} *"));
        assert!(app.sub_title.contains("main.py"));
        assert!(app.sub_title.contains("utf-8"));
    }

    #[test]
    fn portable_env_defauts() {
        let dir = TempDir::new().unwrap();
        let app = App::new(dir.path().to_path_buf()).unwrap();
        let env = app.portable_env(HashMap::new());
        let expected_root = canonical_root(dir.path());
        assert_eq!(
            env.get("PIP_CACHE_DIR").unwrap(),
            &expected_root
                .join("cache")
                .join("pip")
                .display()
                .to_string()
        );
        assert_eq!(
            env.get("PYTHONPYCACHEPREFIX").unwrap(),
            &expected_root
                .join("cache")
                .join("pycache")
                .display()
                .to_string()
        );
        assert_eq!(
            env.get("TEMP").unwrap(),
            &expected_root.join("tmp").display().to_string()
        );
        assert_eq!(
            env.get("TMP").unwrap(),
            &expected_root.join("tmp").display().to_string()
        );
        assert_eq!(env.get("PYTHONNOUSERSITE").unwrap(), "1");
        assert_eq!(
            env.get("CODEX_HOME").unwrap(),
            &expected_root.join("codex_home").display().to_string()
        );
        assert_eq!(
            env.get("NPM_CONFIG_CACHE").unwrap(),
            &expected_root
                .join("cache")
                .join("npm")
                .display()
                .to_string()
        );
        assert_eq!(env.get("NPM_CONFIG_UPDATE_NOTIFIER").unwrap(), "false");
    }

    #[test]
    fn sanitize_codex_env_supprime() {
        let dir = TempDir::new().unwrap();
        let app = App::new(dir.path().to_path_buf()).unwrap();
        with_env_lock(|| {
            let mut env = HashMap::from([
                ("OPENAI_API_KEY".to_string(), "sk-test".to_string()),
                ("CODEX_API_KEY".to_string(), "sk-codex".to_string()),
                (
                    "OPENAI_BASE_URL".to_string(),
                    "https://example.com".to_string(),
                ),
            ]);
            remove_env("USBIDE_CODEX_ALLOW_API_KEY");
            remove_env("USBIDE_CODEX_ALLOW_CUSTOM_BASE");
            app.sanitize_codex_env(&mut env);
            assert!(!env.contains_key("OPENAI_API_KEY"));
            assert!(!env.contains_key("CODEX_API_KEY"));
            assert!(!env.contains_key("OPENAI_BASE_URL"));
        });
    }

    #[test]
    fn sanitize_codex_env_respecte_overrides() {
        let dir = TempDir::new().unwrap();
        let app = App::new(dir.path().to_path_buf()).unwrap();
        with_env_lock(|| {
            let mut env = HashMap::from([
                ("OPENAI_API_KEY".to_string(), "sk-test".to_string()),
                ("CODEX_API_KEY".to_string(), "sk-codex".to_string()),
                (
                    "OPENAI_BASE_URL".to_string(),
                    "https://example.com".to_string(),
                ),
            ]);
            set_env("USBIDE_CODEX_ALLOW_API_KEY", "1");
            set_env("USBIDE_CODEX_ALLOW_CUSTOM_BASE", "true");
            app.sanitize_codex_env(&mut env);
            assert_eq!(env.get("OPENAI_API_KEY").unwrap(), "sk-test");
            assert_eq!(env.get("CODEX_API_KEY").unwrap(), "sk-codex");
            assert_eq!(env.get("OPENAI_BASE_URL").unwrap(), "https://example.com");
            remove_env("USBIDE_CODEX_ALLOW_API_KEY");
            remove_env("USBIDE_CODEX_ALLOW_CUSTOM_BASE");
        });
    }

    #[test]
    fn codex_flags_env() {
        let dir = TempDir::new().unwrap();
        let app = App::new(dir.path().to_path_buf()).unwrap();
        with_env_lock(|| {
            set_env("USBIDE_CODEX_DEVICE_AUTH", "1");
            assert!(app.codex_device_auth_enabled());
            set_env("USBIDE_CODEX_DEVICE_AUTH", "0");
            assert!(!app.codex_device_auth_enabled());
            remove_env("USBIDE_CODEX_DEVICE_AUTH");
            set_env("USBIDE_CODEX_AUTO_INSTALL", "0");
            assert!(!app.codex_auto_install_enabled());
            set_env("USBIDE_CODEX_AUTO_INSTALL", "1");
            assert!(app.codex_auto_install_enabled());
            remove_env("USBIDE_CODEX_AUTO_INSTALL");
            set_env("USBIDE_CODEX_SANDBOX", "workspace-write");
            set_env("USBIDE_CODEX_APPROVAL", "never");
            let app2 = App::new(dir.path().to_path_buf()).unwrap();
            assert_eq!(app2.codex_sandbox_mode, CodexSandboxMode::WorkspaceWrite);
            assert_eq!(app2.codex_approval_policy, CodexApprovalPolicy::Never);
            remove_env("USBIDE_CODEX_SANDBOX");
            remove_env("USBIDE_CODEX_APPROVAL");
        });
    }

    #[test]
    fn record_issue_cree_bug_md() {
        let dir = TempDir::new().unwrap();
        let app = App::new(dir.path().to_path_buf()).unwrap();
        app.record_issue("erreur", "Erreur test", "test_unitaire", None);
        let contenu = fs::read_to_string(dir.path().join("bug.md")).unwrap();
        assert!(contenu.contains("niveau: erreur"));
        assert!(contenu.contains("contexte: test_unitaire"));
        assert!(contenu.contains("message: Erreur test"));
    }
}
