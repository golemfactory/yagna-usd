use anyhow::{anyhow, bail};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use strum_macros::{Display, EnumString, EnumVariantNames, IntoStaticStr};

use tokio::process::Command;
use ya_core_model::payment::local::{
    InvoiceStats, InvoiceStatusNotes, NetworkName, StatusNotes, StatusResult,
};
use ya_core_model::version::VersionInfo;

pub struct VersionRaw {
    pub version: String,
    pub sha: String,
    pub date: String,
    pub build: String,
}

pub struct PaymentPlatform {
    pub platform: &'static str,
    pub driver: &'static str,
    pub token: &'static str,
}

pub struct PaymentDriver(pub HashMap<&'static str, PaymentPlatform>);

lazy_static! {
    pub static ref ZKSYNC_DRIVER: PaymentDriver = {
        let mut zksync = HashMap::new();
        zksync.insert(
            NetworkName::Mainnet.into(),
            PaymentPlatform {
                platform: "zksync-mainnet-glm",
                driver: "zksync",
                token: "GLM",
            },
        );
        zksync.insert(
            NetworkName::Rinkeby.into(),
            PaymentPlatform {
                platform: "zksync-rinkeby-tglm",
                driver: "zksync",
                token: "tGLM",
            },
        );
        PaymentDriver(zksync)
    };
    pub static ref ERC20_DRIVER: PaymentDriver = {
        let mut erc20 = HashMap::new();
        erc20.insert(
            NetworkName::Mainnet.into(),
            PaymentPlatform {
                platform: "erc20-mainnet-glm",
                driver: "erc20",
                token: "GLM",
            },
        );
        erc20.insert(
            NetworkName::Rinkeby.into(),
            PaymentPlatform {
                platform: "erc20-rinkeby-tglm",
                driver: "erc20",
                token: "tGLM",
            },
        );
        erc20.insert(
            NetworkName::Goerli.into(),
            PaymentPlatform {
                platform: "erc20-goerli-tglm",
                driver: "erc20",
                token: "tGLM",
            },
        );
        erc20.insert(
            NetworkName::Mumbai.into(),
            PaymentPlatform {
                platform: "erc20-mumbai-tglm",
                driver: "erc20",
                token: "tGLM",
            },
        );
        erc20.insert(
            NetworkName::Polygon.into(),
            PaymentPlatform {
                platform: "erc20-polygon-glm",
                driver: "erc20",
                token: "GLM",
            },
        );
        PaymentDriver(erc20)
    };
}

impl PaymentDriver {
    pub fn platform(&self, network: &NetworkName) -> anyhow::Result<&PaymentPlatform> {
        let net: &str = network.into();
        Ok(self.0.get(net).ok_or(anyhow!(
            "Payment driver config for network '{}' not found.",
            network
        ))?)
    }
}

#[derive(
    Clone,
    Debug,
    Deserialize,
    Display,
    EnumVariantNames,
    EnumString,
    Eq,
    Hash,
    IntoStaticStr,
    PartialEq,
    Serialize,
)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum NetworkGroup {
    Mainnet,
    Testnet,
}

lazy_static! {
    pub static ref NETWORK_GROUP_MAP: HashMap<NetworkGroup, Vec<NetworkName>> = {
        let mut ngm = HashMap::new();
        ngm.insert(
            NetworkGroup::Mainnet,
            vec![NetworkName::Mainnet, NetworkName::Polygon],
        );
        ngm.insert(
            NetworkGroup::Testnet,
            vec![
                NetworkName::Rinkeby,
                NetworkName::Mumbai,
                NetworkName::Goerli,
            ],
        );
        ngm
    };
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Id {
    pub node_id: String,
}

pub trait PaymentSummary {
    fn total_pending(&self) -> (BigDecimal, u64);
    fn unconfirmed(&self) -> (BigDecimal, u64);
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivityStatus {
    pub last1h: HashMap<String, u64>,
    pub total: HashMap<String, u64>,
    pub last_activity_ts: Option<DateTime<Utc>>,
}

impl ActivityStatus {
    pub fn last1h_processed(&self) -> u64 {
        self.last1h.get("Terminated").copied().unwrap_or_default()
    }

    pub fn in_progress(&self) -> u64 {
        let mut in_progress = 0;
        for (k, v) in &self.last1h {
            if k != "Terminated" && k != "New" {
                in_progress += *v;
            }
        }
        in_progress
    }

    pub fn total_processed(&self) -> u64 {
        self.total.get("Terminated").copied().unwrap_or_default()
    }
}
impl PaymentSummary for StatusNotes {
    fn total_pending(&self) -> (BigDecimal, u64) {
        (
            &self.accepted.total_amount - &self.confirmed.total_amount,
            self.accepted.agreements_count - self.confirmed.agreements_count,
        )
    }

    fn unconfirmed(&self) -> (BigDecimal, u64) {
        (
            &self.requested.total_amount - &self.accepted.total_amount,
            self.requested.agreements_count - self.accepted.agreements_count,
        )
    }
}

impl PaymentSummary for InvoiceStatusNotes {
    fn total_pending(&self) -> (BigDecimal, u64) {
        let value = self.accepted.clone();
        (value.total_amount, value.agreements_count)
    }

    fn unconfirmed(&self) -> (BigDecimal, u64) {
        let value = self.issued.clone() + self.received.clone();
        (value.total_amount.clone(), value.agreements_count)
    }
}

pub struct YagnaCommand {
    pub(super) cmd: Command,
}

impl YagnaCommand {
    async fn run(self) -> anyhow::Result<Vec<u8>> {
        let mut cmd = self.cmd;
        log::debug!("Running: {:?}", cmd);
        let output = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(anyhow::anyhow!(
                "{:?} failed.: Stdout:\n{}\nStderr:\n{}",
                cmd,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    async fn run_json<T: DeserializeOwned>(mut self) -> anyhow::Result<T> {
        self.cmd.args(&["--json"]);
        let stdout = self.run().await?;
        Ok(serde_json::from_slice(&stdout)?)
    }

    pub async fn default_id(mut self) -> anyhow::Result<Id> {
        self.cmd.args(&["id", "show"]);
        let output: Result<Id, String> = self.run_json().await?;
        output.map_err(anyhow::Error::msg)
    }

    pub async fn version(mut self) -> anyhow::Result<VersionInfo> {
        self.cmd.args(&["version", "show"]);
        self.run_json().await
    }

    pub async fn version_raw(mut self) -> anyhow::Result<VersionRaw> {
        self.cmd.args(&["--version"]);
        let output = self.run().await?;
        let re = Regex::new(r"yagna ([0-9.]+) \(([a-z0-9]+) ([-0-9]+)( build #(\d+))?")?;
        if let Some(cap) = re.captures(&String::from_utf8_lossy(&output)) {
            Ok(VersionRaw {
                version: cap[1].to_string(),
                sha: cap[2].to_string(),
                date: cap[3].to_string(),
                build: cap.get(5).map(|m| m.as_str()).unwrap_or_default().to_string(),
            })
        } else {
            bail!("cannot parse yagna version {:?}", output)
        }
    }

    pub async fn payment_status(
        mut self,
        address: &str,
        network: &NetworkName,
        payment_driver: &PaymentDriver,
    ) -> anyhow::Result<StatusResult> {
        self.cmd.args(&["payment", "status"]);
        self.cmd.args(&["--account", address]);

        let payment_platform = payment_driver.platform(network)?;
        self.cmd.args(&["--network", &network.to_string()]);
        self.cmd.args(&["--driver", payment_platform.driver]);

        self.run_json().await
    }

    pub async fn invoice_status(mut self) -> anyhow::Result<InvoiceStats> {
        self.cmd.args(&["payment", "invoice", "status"]);
        self.run_json().await
    }

    pub async fn activity_status(mut self) -> anyhow::Result<ActivityStatus> {
        self.cmd.args(&["activity", "status"]);
        self.run_json().await
    }
}
