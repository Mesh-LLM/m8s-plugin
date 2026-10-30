//! App-owned signing bridge. The launch plan contains an expiring capability,
//! never an owner key. All requests are loopback and bound to an exact target.
use super::owner::OwnerClient;
use m8s_bootstrap::{CertificateClaims, NodeCertificate, NodeSigner};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
use zeroize::Zeroizing;
type Result<T> = std::result::Result<T, &'static str>;
const ERROR: &str = "current-mesh-attachment-refused";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Bridge {
    pub port: u16,
    capability: String,
    target: Value,
}
pub(super) struct Attached {
    bridge: Bridge,
    pub port: u16,
    pub signing_key: [u8; 32],
    pub owner_id: String,
    identity: Value,
}
pub(super) enum Client {
    Dedicated(OwnerClient),
    Token(super::token_join::TokenClient),
    Attached(Attached),
}
impl Bridge {
    fn call(&self, body: Value) -> Result<Value> {
        use std::io::Read;
        if self.port == 0
            || self.capability.len() != 128
            || !self.capability.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(ERROR);
        }
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| ERROR)?;
        let response = client
            .post(format!("http://127.0.0.1:{}/app/compute/bridge", self.port))
            .bearer_auth(&self.capability)
            .json(&body)
            .send()
            .map_err(|_| ERROR)?;
        if !response.status().is_success() {
            return Err(ERROR);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| ERROR)?;
        if bytes.len() > 65536 {
            return Err(ERROR);
        }
        serde_json::from_slice(&bytes).map_err(|_| ERROR)
    }
}
impl Attached {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = Zeroizing::new(super::super::models::load_bytes(path).map_err(|_| ERROR)?);
        if bytes.len() > 8192 {
            return Err(ERROR);
        }
        let bridge: Bridge = serde_json::from_slice(&bytes).map_err(|_| ERROR)?;
        let info = bridge.call(json!({"operation":"attach"}))?;
        if info["target"] != bridge.target {
            return Err(ERROR);
        }
        let port = info["api_port"]
            .as_u64()
            .and_then(|p| u16::try_from(p).ok())
            .filter(|p| *p != 0)
            .ok_or(ERROR)?;
        let signing_key = hex::decode(info["owner_signing_key"].as_str().ok_or(ERROR)?)
            .map_err(|_| ERROR)?
            .try_into()
            .map_err(|_| ERROR)?;
        let owner_id = info["owner_id"].as_str().ok_or(ERROR)?.to_owned();
        let identity = json!({"schema":"m8s.desktop-owner/v1","api_port":port,"console_port":info["console_port"],
            "node_id":info["target"]["node_id"],"owner_id":owner_id,"cert_id":info["target"]["cert_id"],
            "mesh_id":info["target"]["mesh_id"],"expires_at":info["expires_at"]});
        Ok(Self {
            bridge,
            port,
            signing_key,
            owner_id,
            identity,
        })
    }
    pub fn invite(&self) -> Result<Zeroizing<Vec<u8>>> {
        let info = self.bridge.call(json!({"operation":"attach"}))?;
        if info["target"] != self.bridge.target
            || info["owner_id"].as_str() != Some(&self.owner_id)
            || info["owner_signing_key"].as_str() != Some(hex::encode(self.signing_key).as_str())
        {
            return Err(ERROR);
        }
        let token = info["invite"]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 16384)
            .ok_or(ERROR)?;
        Ok(Zeroizing::new(token.as_bytes().to_vec()))
    }
}
impl NodeSigner for Attached {
    fn sign(&mut self, claims: &CertificateClaims) -> m8s_bootstrap::Result<NodeCertificate> {
        let result = tokio::task::block_in_place(|| {
            self.bridge
                .call(json!({"operation":"sign","node":claims.node.0,
            "issued_at":claims.issued_at,"expires_at":claims.expires_at}))
        })
        .map_err(|_| m8s_bootstrap::Error::Certificate)?;
        serde_json::to_vec(&result["certificate"])
            .map(NodeCertificate)
            .map_err(|_| m8s_bootstrap::Error::Certificate)
    }
}
impl Client {
    pub async fn invite(&mut self) -> Result<Zeroizing<Vec<u8>>> {
        match self {
            Self::Dedicated(client) => client.invite().await,
            Self::Token(client) => tokio::task::block_in_place(|| client.invite()),
            Self::Attached(client) => tokio::task::block_in_place(|| client.invite()),
        }
    }
    pub fn port(&self) -> u16 {
        match self {
            Self::Dedicated(client) => client.port,
            Self::Token(client) => client.port(),
            Self::Attached(client) => client.port,
        }
    }
    pub fn desktop_attachment(&self) -> Value {
        match self {
            Self::Dedicated(client) => client.desktop_attachment(),
            Self::Token(client) => client.identity(),
            Self::Attached(client) => client.identity.clone(),
        }
    }
    pub async fn stop(&mut self) -> Result<()> {
        match self {
            Self::Dedicated(client) => client.stop().await,
            Self::Attached(_) | Self::Token(_) => Ok(()),
        }
    }
}

pub(super) enum Signer<'a> {
    Dedicated(m8s_bootstrap::mp001::signing::Mp001NodeSigner<'a>),
    Attached(&'a mut Attached),
}
impl NodeSigner for Signer<'_> {
    fn sign(&mut self, claims: &CertificateClaims) -> m8s_bootstrap::Result<NodeCertificate> {
        match self {
            Self::Dedicated(signer) => signer.sign(claims),
            Self::Attached(signer) => signer.sign(claims),
        }
    }
}
