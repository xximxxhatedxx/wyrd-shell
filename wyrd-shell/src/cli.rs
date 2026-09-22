//! CLI arguments.

use clap::Parser;
use std::path::PathBuf;

fn default_config_path() -> PathBuf {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config_home).join("wyrd/init.lua")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/wyrd/init.lua")
    } else {
        PathBuf::from(".config/wyrd/init.lua")
    }
}

#[derive(Debug, Parser)]
#[command(name = "wyrd-shell")]
#[command(about = "Wyrd Wayland desktop shell")]
pub struct Cli {
    /// Path to init.lua
    #[arg(short, long, default_value_os_t = default_config_path())]
    pub config: PathBuf,

    /// Run as daemon
    #[arg(long)]
    pub daemonize: bool,

    /// Enable debug overlay
    #[arg(long)]
    pub debug: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Render backend: 'auto' or 'cpu'.
    #[arg(long, value_enum, default_value_t = RenderModeArg::Auto)]
    pub render_mode: RenderModeArg,

    /// Send an action command to the running wyrd-shell instance and exit (e.g. 'popup:launcher', 'popup:toggle calendar', 'theme:set catppuccin')
    #[arg(long)]
    pub action: Option<String>,

    /// Switch active theme ('catppuccin', 'aetheria', 'nord', 'gruvbox', 'tokyo-night')
    #[arg(long)]
    pub theme: Option<String>,

    /// Switch active bar layout ('top', 'left')
    #[arg(long)]
    pub layout: Option<String>,

    /// Output shell completions for specified shell ('bash', 'zsh', 'fish')
    #[arg(long, value_parser = ["bash", "zsh", "fish"])]
    pub completions: Option<String>,

    /// Target compositor integration (auto detects running compositor)
    #[arg(long, value_enum, default_value_t = CompositorArg::Auto)]
    pub compositor: CompositorArg,

    /// Run system and environment diagnostics
    #[arg(long)]
    pub doctor: bool,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, PartialEq, Eq, Default)]
pub enum CompositorArg {
    #[default]
    Auto,
    Hyprland,
    Niri,
    Sway,
    None,
}

impl From<CompositorArg> for wyrd_engine::compositor::CompositorChoice {
    fn from(arg: CompositorArg) -> Self {
        match arg {
            CompositorArg::Auto => wyrd_engine::compositor::CompositorChoice::Auto,
            CompositorArg::Hyprland => wyrd_engine::compositor::CompositorChoice::Hyprland,
            CompositorArg::Niri => wyrd_engine::compositor::CompositorChoice::Niri,
            CompositorArg::Sway => wyrd_engine::compositor::CompositorChoice::Sway,
            CompositorArg::None => wyrd_engine::compositor::CompositorChoice::None,
        }
    }
}

#[derive(Clone, Copy, Debug, clap::ValueEnum, PartialEq, Eq, Default)]
pub enum RenderModeArg {
    #[default]
    Auto,
    Cpu,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_mode_arg_parsing() {
        let cli_auto = Cli::try_parse_from(["wyrd-shell", "--render-mode", "auto"]).unwrap();
        assert_eq!(cli_auto.render_mode, RenderModeArg::Auto);

        let cli_cpu = Cli::try_parse_from(["wyrd-shell", "--render-mode", "cpu"]).unwrap();
        assert_eq!(cli_cpu.render_mode, RenderModeArg::Cpu);

        // Passing --render-mode=gpu MUST be rejected by clap
        let cli_gpu = Cli::try_parse_from(["wyrd-shell", "--render-mode", "gpu"]);
        assert!(cli_gpu.is_err(), "--render-mode=gpu must be rejected");
    }
}
