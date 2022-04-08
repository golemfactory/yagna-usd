//! Subcommand execution handling

use directories::UserDirs;
use std::path::Path;
use std::{env, fs};
use tokio::process::Command;

mod provider;
mod yagna;

pub use provider::*;
pub use yagna::*;

pub struct YaCommand {
    base_path: Option<Box<Path>>,
}

impl YaCommand {
    pub fn new() -> anyhow::Result<Self> {
        let mut me = env::current_exe()?;

        // find original binary path.
        for _ in 0..5 {
            if let Ok(base) = fs::read_link(&me) {
                me = base;
            } else {
                break;
            }
        }

        let base_path = me
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Unable to resolve yagna binaries location"))?;

        if !base_path.join("yagna").exists() || !base_path.join("ya-provider").exists() {
            return Ok(Self { base_path: None });
        }

        Ok(Self {
            base_path: Some(base_path.into()),
        })
    }

    pub fn cmd(&self, program: &str) -> Command {
        match &self.base_path {
            Some(path) => Command::new(path.join(program)),
            None => Command::new(program),
        }
    }

    pub fn ya_provider(&self) -> anyhow::Result<YaProviderCommand> {
        let mut cmd = self.cmd("ya-provider");

        if let Some(user_dirs) = UserDirs::new() {
            let plugins_dir = user_dirs.home_dir().join(".local/lib/yagna/plugins");
            if plugins_dir.exists() {
                cmd.env("EXE_UNIT_PATH", plugins_dir.join("ya-*.json"));
            }
        }

        Ok(YaProviderCommand { cmd })
    }

    pub fn yagna(&self) -> anyhow::Result<YagnaCommand> {
        let cmd = self.cmd("yagna");
        Ok(YagnaCommand { cmd })
    }
}
