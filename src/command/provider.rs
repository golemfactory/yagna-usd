use anyhow::Context;
use serde::Deserialize;
use std::{collections::BTreeMap, process::Stdio};
use tokio::process::Command;
use ya_core_model::NodeId;

pub struct YaProviderCommand {
    pub(super) cmd: Command,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Preset {
    pub name: String,
    pub exeunit_name: String,
    pub initial_price: f64,
    pub usage_coeffs: UsageDef,
}

#[derive(Deserialize)]
pub struct ProviderConfig {
    pub node_name: Option<String>,
    pub subnet: Option<String>,
    pub account: Option<NodeId>,
}


pub type UsageDef = BTreeMap<String, f64>;

#[derive(Deserialize)]
pub struct RuntimeInfo {
    pub name: String,
    pub description: Option<String>,
}

impl YaProviderCommand {
    pub async fn get_config(mut self) -> anyhow::Result<ProviderConfig> {
        let output = self
            .cmd
            .args(&["--json", "config", "get"])
            .stderr(Stdio::inherit())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .output()
            .await
            .context(format!("failed to get ya-provider configuration {:?}", self.cmd))?;

        serde_json::from_slice(output.stdout.as_slice()).context("parsing ya-provider config get")
    }

}
