use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone, Debug)]
enum UiMode {
    Gui,
    Tui,
}

#[derive(Parser)]
#[command(name = "usbide", about = "Mini IDE terminal portable (Rust).")]
struct Args {
    /// Dossier racine du workspace (par defaut: repertoire courant).
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Type d'interface: gui (fenetre) ou tui (terminal).
    #[arg(long, value_enum, default_value_t = UiMode::Gui)]
    ui: UiMode,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.ui {
        UiMode::Gui => ide_usb::gui::run(args.root),
        UiMode::Tui => {
            if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
                eprintln!("Interface terminal (TUI) : aucun TTY detecte.");
                eprintln!(
                    "Lance l'app dans un vrai terminal (Windows Terminal / PowerShell / cmd)."
                );
                eprintln!(
                    "Dans RustRover : active 'Emulate terminal in output console' ou 'Run in Terminal'."
                );
                return Ok(());
            }
            ide_usb::ui::run(args.root)
        }
    }
}
