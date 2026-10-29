//! Stock-Mesh admission is explicitly weaker than owner certification. Local
//! target and private token are operator-selected; no certificate claim is made.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const ERROR: &str = "token-join-target-refused";
type Result<T> = std::result::Result<T, &'static str>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema: String,
    admission: String,
    join_file: PathBuf,
    api_port: u16,
    console_port: u16,
    node_id: String,
    mesh_id: String,
}
pub(super) struct TokenClient {
    selected: Selection,
    token: Zeroizing<Vec<u8>>,
}
fn private_bytes(path: &Path) -> Result<Vec<u8>> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path).map_err(|_| ERROR)?;
    if !path.is_absolute()
        || path.canonicalize().map_err(|_| ERROR)? != path
        || !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
    {
        return Err(ERROR);
    }
    super::super::models::load_bytes(path).map_err(|_| ERROR)
}
impl TokenClient {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = Zeroizing::new(private_bytes(path)?);
        let selected: Selection = serde_json::from_slice(&bytes).map_err(|_| ERROR)?;
        if selected.schema != "m8s.token-target/v0"
            || selected.admission != "token-only-provider-trusted"
            || selected.api_port == 0
            || selected.console_port == 0
            || !matches!(selected.node_id.len(), 10 | 64)
            || !selected.node_id.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(selected.mesh_id.len(), 16 | 32 | 64)
            || !selected.mesh_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(ERROR);
        }
        let mut token = Zeroizing::new(private_bytes(&selected.join_file)?);
        while token.last().is_some_and(|b| matches!(b, b'\n' | b'\r')) {
            token.pop();
        }
        if token.is_empty() || token.len() > 16384 || !token.iter().all(|b| (33..=126).contains(b))
        {
            return Err(ERROR);
        }
        let this = Self { selected, token };
        let status = this.check_target()?;
        if status["is_host"] != false || !status["peers"].as_array().is_some_and(Vec::is_empty) {
            return Err("token-target-must-be-isolated-passive-client");
        }
        Ok(this)
    }
    pub fn check_target(&self) -> Result<Value> {
        let bytes = super::http::status(
            self.selected.console_port,
            Instant::now() + Duration::from_secs(3),
        )
        .map_err(|_| ERROR)?;
        let status: Value = serde_json::from_slice(&bytes).map_err(|_| ERROR)?;
        if status["node_id"] != self.selected.node_id || status["mesh_id"] != self.selected.mesh_id
        {
            return Err(ERROR);
        }
        Ok(status)
    }
    pub fn invite(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.check_target()?;
        Ok(Zeroizing::new(self.token.to_vec()))
    }
    pub fn port(&self) -> u16 {
        self.selected.api_port
    }
    pub fn mesh_id(&self) -> &str {
        &self.selected.mesh_id
    }
    pub fn identity(&self) -> Value {
        json!({"schema":"m8s.token-owner/v0","node_id":self.selected.node_id,"mesh_id":self.selected.mesh_id,"api_port":self.selected.api_port,"console_port":self.selected.console_port,"certificate_verified":false,"admission":"token-only-provider-trusted"})
    }
}

fn sole_worker(status: &Value, id: &str) -> Result<bool> {
    let peers = status["peers"].as_array().ok_or(ERROR)?;
    if status["is_host"] != false || peers.len() > 1 {
        return Err("token-canary-target-ambiguous");
    }
    if let Some(peer) = peers.first() {
        if peer["id"].as_str().or(peer["node_id"].as_str()) != Some(id) {
            return Err("token-worker-mismatch");
        }
        return Ok(true);
    }
    Ok(false)
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn serve(
    client: &TokenClient,
    directory: &Path,
    control: Option<&Path>,
    access: &m8s_provider::vast::readonly::access::NodeAccess,
    host: &m8s_provider::vast::readonly::provisioning_log::HostPublicKey,
    image: &str,
    model: super::demo_model::DemoModel,
    token: Zeroizing<Vec<u8>>,
    expires: i64,
) -> Result<Value> {
    use m8s_provider::vast::provision::{ProvisionPacket, ProvisioningSession};
    let now = super::owner::utc()?;
    let remaining = expires
        .checked_sub(now)
        .filter(|n| *n > 0)
        .ok_or(ERROR)?
        .min(1500) as u64;
    let packet=ProvisionPacket::token_join(&json!({"schema":"m8s.token-join/v0","instance_id":access.instance_id().get(),"image_sha256":image.rsplit_once("@sha256:").ok_or(ERROR)?.1,"model":model.label(),"mesh_id":client.mesh_id(),"lifetime_seconds":remaining}),m8s_secrets::SecretValue::new(token.to_vec())).map_err(|_|ERROR)?;
    let mut session = ProvisioningSession::start(
        access,
        host,
        directory,
        &directory.join("provision-key"),
        packet,
    )
    .await
    .map_err(|_| ERROR)?;
    eprintln!("prototype stage: token-delivered-over-pinned-ssh");
    let complete = std::sync::atomic::AtomicBool::new(false);
    let work = async {
        let worker = session
            .token_identity(Duration::from_secs(remaining))
            .await
            .map_err(|_| ERROR)?;
        let id = worker["node_id"]
            .as_str()
            .filter(|v| matches!(v.len(), 10 | 64) && v.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or(ERROR)?;
        if worker["schema"] != "m8s.token-worker/v0"
            || worker["mesh_id"] != client.mesh_id()
            || worker["certificate_verified"] != false
        {
            return Err(ERROR);
        }
        let deadline = Instant::now()
            + Duration::from_secs(
                expires
                    .checked_sub(super::owner::utc()?)
                    .filter(|n| *n > 0)
                    .ok_or(ERROR)? as u64,
            );
        loop {
            if Instant::now() >= deadline {
                return Err("token-model-not-observed");
            }
            let status = tokio::task::block_in_place(|| client.check_target())?;
            let present = sole_worker(&status, id)?;
            if present {
                let model_result = tokio::task::block_in_place(|| {
                    super::model_wait::wait_exact(
                        client.port(),
                        &[model.request_model(), model.served()],
                        (Instant::now() + Duration::from_secs(1)).min(deadline),
                    )
                    .map(str::to_owned)
                });
                if let Ok(discovered) = model_result {
                    eprintln!("prototype stage: token-worker-model-observed");
                    let status = tokio::task::block_in_place(|| client.check_target())?;
                    if !sole_worker(&status, id)? {
                        return Err("token-worker-left");
                    }
                    let mut request = model.completion_request();
                    request["model"] = json!(discovered);
                    let body = serde_json::to_vec(&request).map_err(|_| ERROR)?;
                    let bytes = tokio::task::block_in_place(|| {
                        super::http::exchange(
                            client.port(),
                            &body,
                            (Instant::now() + Duration::from_secs(60)).min(deadline),
                        )
                    })
                    .map_err(|_| ERROR)?;
                    let answer = super::project(&bytes, model.served())?;
                    let status = tokio::task::block_in_place(|| client.check_target())?;
                    if !sole_worker(&status, id)? {
                        return Err("token-worker-left");
                    }
                    super::owner::private_file(&directory.join("token-ready.json"),&serde_json::to_vec(&json!({"schema":"m8s.token-ready/v0","instance_id":access.instance_id().get(),"mesh_id":client.mesh_id(),"node_id":id,"model":model.served(),"certificate_verified":false})).map_err(|_|ERROR)?)?;
                    eprintln!("prototype stage: token-mesh-inference-verified");
                    complete.store(true, std::sync::atomic::Ordering::Release);
                    while Instant::now() < deadline
                        && !control.is_some_and(super::demo_control::requested)
                    {
                        let status = tokio::task::block_in_place(|| client.check_target())?;
                        if !sole_worker(&status, id)? {
                            return Err("token-worker-left");
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    return Ok(
                        json!({"content":answer["content"],"finish_reason":answer["finish_reason"],"remote_node_id":id,"certificate_verified":false}),
                    );
                }
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    };
    let cancelled = async {
        loop {
            if complete.load(std::sync::atomic::Ordering::Acquire) {
                return std::future::pending().await;
            }
            if super::owner::utc().unwrap_or(i64::MAX) >= expires
                || control.is_some_and(super::demo_control::requested)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    };
    let result = tokio::select! {result=work=>result, _=cancelled=>Err("token-work-cancelled")};
    let _ = session.stop().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn token_files_require_private_owned_regular_paths() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let file = root.join("invite");
        std::fs::write(&file, b"synthetic").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(private_bytes(&file).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(private_bytes(&file).unwrap(), b"synthetic");
        std::os::unix::fs::symlink(&file, root.join("alias")).unwrap();
        assert!(private_bytes(&root.join("alias")).is_err());
    }
    #[test]
    fn canary_attribution_refuses_other_or_multiple_hosts() {
        assert_eq!(
            sole_worker(&json!({"is_host":false,"peers":[]}), "worker"),
            Ok(false)
        );
        assert_eq!(
            sole_worker(
                &json!({"is_host":false,"peers":[{"id":"worker"}]}),
                "worker"
            ),
            Ok(true)
        );
        for value in [
            json!({"is_host":true,"peers":[{"id":"worker"}]}),
            json!({"is_host":false,"peers":[{"id":"other"}]}),
            json!({"is_host":false,"peers":[{"id":"worker"},{"id":"other"}]}),
            json!({"peers":[]}),
        ] {
            assert!(sole_worker(&value, "worker").is_err());
        }
    }
    #[test]
    fn acknowledgement_is_not_implicit() {
        let value = json!({"schema":"m8s.token-target/v0","join_file":"/private/synthetic","api_port":9337,"console_port":3131,"node_id":"a".repeat(10),"mesh_id":"b".repeat(16)});
        assert!(serde_json::from_value::<Selection>(value).is_err());
    }
}
