//! Restricted owner-to-node SSH provisioning. TLS bootstrap still separately
//! authenticates the owner/node and consumes the nonce before native startup.
use crate::vast::readonly::{access::NodeAccess, provisioning_log::HostPublicKey};
use crate::{Error, Result};
use m8s_secrets::SecretValue;
use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    num::NonZeroU64,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
};

const MAGIC: &[u8] = b"M8S-PROVISION-V1\n";
const FIELDS: &[&str] = &[
    "schema",
    "instance_id",
    "container_id",
    "image_sha256",
    "owner_endpoint",
    "owner_signing_key",
    "mesh_owner_id",
    "owner_address",
    "bind_address",
    "trust",
    "program",
    "gguf",
    "material_root",
    "model",
    "heartbeat_ttl_seconds",
    "max_lifetime_seconds",
    "port",
    "console_port",
];

pub use super::provision_protocol::{FLINT_MODEL_REF, QWEN38_MODEL_REF};

/// One-use secret frame. Only public configuration is encoded as JSON; the
/// nonce and node key are appended in the receiver's fixed raw wire format.
pub struct ProvisionPacket {
    frame: SecretValue,
    instance: NonZeroU64,
    image: String,
    command: &'static str,
}
impl ProvisionPacket {
    pub fn new(config: &Value, nonce: SecretValue, node_key: SecretValue) -> Result<Self> {
        let object = config.as_object().ok_or(Error::Invalid)?;
        let instance = config
            .get("instance_id")
            .and_then(Value::as_u64)
            .and_then(NonZeroU64::new)
            .ok_or(Error::Invalid)?;
        let image = config
            .get("image_sha256")
            .and_then(Value::as_str)
            .ok_or(Error::Invalid)?;
        if object.len() != FIELDS.len()
            || FIELDS.iter().any(|name| !object.contains_key(*name))
            || config["schema"] != "m8s.vast-node/v1"
            || config["container_id"] != instance.to_string()
            || config["program"] != "/opt/mesh/mesh-llm"
            || !(matches!(
                (config["model"].as_str(), config["gguf"].as_str()),
                (
                    Some("qwen2.5-0.5b-instruct-q4_k_m"),
                    Some("/models/qwen2.5-0.5b-instruct-q4_k_m.gguf")
                )
            ) || ([FLINT_MODEL_REF, QWEN38_MODEL_REF]
                .iter()
                .any(|model| config["model"] == *model)
                && config["gguf"].is_null()))
            || config["material_root"] != "/var/lib/m8s/material"
            || nonce.expose().len() != 32
            || node_key.expose().len() != 32
        {
            return Err(Error::Invalid);
        }
        for name in [
            "image_sha256",
            "owner_endpoint",
            "owner_signing_key",
            "mesh_owner_id",
        ] {
            let text = config[name].as_str().ok_or(Error::Invalid)?;
            if text.len() != 64
                || !text
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(Error::Invalid);
            }
        }
        let public = serde_json::to_vec(config).map_err(|_| Error::Invalid)?;
        if public.is_empty() || public.len() > 8192 {
            return Err(Error::Invalid);
        }
        // Reserve the exact final length BEFORE appending any secret bytes.
        let mut frame = Vec::with_capacity(MAGIC.len() + 4 + public.len() + 32 + 64);
        frame.extend_from_slice(MAGIC);
        frame.extend_from_slice(&(public.len() as u32).to_be_bytes());
        frame.extend_from_slice(&public);
        frame.extend_from_slice(nonce.expose());
        for byte in node_key.expose() {
            frame.push(b"0123456789abcdef"[(byte >> 4) as usize]);
            frame.push(b"0123456789abcdef"[(byte & 15) as usize]);
        }
        Ok(Self {
            frame: SecretValue::new(frame),
            instance,
            image: image.into(),
            command: "provision-v1",
        })
    }
    /// Explicit token-only admission over the same pinned SSH transport.
    /// This packet is never accepted by the certified receiver.
    pub fn token_join(config: &Value, token: SecretValue) -> Result<Self> {
        let fields = [
            "schema",
            "instance_id",
            "image_sha256",
            "model",
            "mesh_id",
            "lifetime_seconds",
        ];
        let object = config.as_object().ok_or(Error::Invalid)?;
        let instance = config["instance_id"]
            .as_u64()
            .and_then(NonZeroU64::new)
            .ok_or(Error::Invalid)?;
        let image = config["image_sha256"].as_str().ok_or(Error::Invalid)?;
        let mesh = config["mesh_id"].as_str().ok_or(Error::Invalid)?;
        if object.len() != fields.len()
            || fields.iter().any(|f| !object.contains_key(*f))
            || config["schema"] != "m8s.token-join/v0"
            || config["model"] != QWEN38_MODEL_REF
            || image.len() != 64
            || !image.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(mesh.len(), 16 | 32 | 64)
            || !mesh.bytes().all(|b| b.is_ascii_hexdigit())
            || !config["lifetime_seconds"]
                .as_u64()
                .is_some_and(|n| (1..=1500).contains(&n))
            || token.expose().is_empty()
            || token.expose().len() > 16384
            || !token.expose().iter().all(|b| (33..=126).contains(b))
        {
            return Err(Error::Invalid);
        }
        let public = serde_json::to_vec(config).map_err(|_| Error::Invalid)?;
        let magic = b"M8S-TOKEN-JOIN-V0\n";
        let mut frame = Vec::with_capacity(magic.len() + 8 + public.len() + token.expose().len());
        frame.extend_from_slice(magic);
        frame.extend_from_slice(&(public.len() as u32).to_be_bytes());
        frame.extend_from_slice(&public);
        frame.extend_from_slice(&(token.expose().len() as u32).to_be_bytes());
        frame.extend_from_slice(token.expose());
        Ok(Self {
            frame: SecretValue::new(frame),
            instance,
            image: image.into(),
            command: "join-v0",
        })
    }
}

/// Retains the SSH child during bootstrap/model work. Dropping it kills only
/// this client process; independently owned provider cleanup remains mandatory.
/// A returned session is NOT an authentication, bootstrap, or Ready receipt.
pub struct ProvisioningSession {
    child: Child,
}

/// Read bounded observations using the same pinned endpoint and restricted key.
/// This is not an admission, model-readiness, ownership or cleanup receipt.
pub async fn token_status(
    access: &NodeAccess,
    host: &HostPublicKey,
    root: &Path,
    identity: &Path,
) -> Result<Value> {
    if !host.matches_access(access)
        || !literal_path(identity)
        || identity.parent() != Some(root)
        || identity.canonicalize().map_err(|_| Error::Invalid)? != identity
    {
        return Err(Error::Invalid);
    }
    let _directory = private_directory(root)?;
    let key = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(identity)
        .map_err(|_| Error::Invalid)?;
    private_file(&key)?;
    let known = root.join("provision-known-hosts");
    let mut pinned = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&known)
        .map_err(|_| Error::Invalid)?;
    private_file(&pinned)?;
    if pinned.metadata().map_err(|_| Error::Invalid)?.len() > 1024 {
        return Err(Error::Invalid);
    }
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(&mut pinned, 1025), &mut contents)
        .map_err(|_| Error::Invalid)?;
    let address = access.ssh_address();
    if contents
        != format!(
            "[{}]:{} {}\n",
            address.ip(),
            address.port(),
            host.as_openssh()
        )
    {
        return Err(Error::Invalid);
    }
    let mut child = command_for("status-v0", root, identity, &known, address)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Error::Unavailable)?;
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let stdout = child.stdout.take().ok_or(Error::Unavailable)?;
        let mut bytes = Vec::new();
        stdout
            .take(8193)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| Error::Unavailable)?;
        if bytes.len() > 8192 {
            return Err(Error::Invalid);
        }
        if !child
            .wait()
            .await
            .map_err(|_| Error::Unavailable)?
            .success()
        {
            return Err(Error::Unavailable);
        }
        project_token_status(&serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?)
    })
    .await
    .map_err(|_| Error::Unavailable)
    .and_then(|v| v);
    let _ = child.kill().await;
    result
}

fn project_token_status(value: &Value) -> Result<Value> {
    let phase = value["phase"]
        .as_str()
        .filter(|s| {
            matches!(
                *s,
                "awaiting_join"
                    | "unknown"
                    | "starting"
                    | "waiting_for_mesh"
                    | "joined"
                    | "stopped"
            )
        })
        .ok_or(Error::Invalid)?;
    if value["schema"] != "m8s.token-worker-status/v0" {
        return Err(Error::Invalid);
    }
    let mut result = serde_json::json!({"schema":"m8s.token-worker-status/v0","phase":phase});
    for key in [
        "elapsed_seconds",
        "download_restart_count",
        "cache_file_bytes",
        "cache_allocated_bytes",
        "cache_file_count",
        "cache_incomplete_files",
        "worker_rss_bytes",
        "worker_read_bytes",
        "worker_write_bytes",
        "network_received_bytes",
        "network_sent_bytes",
        "requested_model_count",
        "hosted_model_count",
        "runtime_model_count",
        "runtime_instance_count",
        "runtime_error_count",
    ] {
        if let Some(n) = value[key].as_u64() {
            result[key] = n.into();
        }
    }
    for key in [
        "worker_running",
        "api_reachable",
        "mesh_matches",
        "cache_scan_truncated",
        "runtime_snapshot_truncated",
        "runtime_error_download",
        "runtime_error_timeout",
        "runtime_error_connection",
        "runtime_error_auth",
        "runtime_error_rate_limit",
        "runtime_error_disk",
        "runtime_error_memory",
        "runtime_error_native",
        "node_loading",
        "node_serving",
    ] {
        if let Some(v) = value[key].as_bool() {
            result[key] = v.into();
        }
    }
    if let Some(code) = value["worker_exit_code"]
        .as_i64()
        .filter(|n| (-255..=255).contains(n))
    {
        result["worker_exit_code"] = code.into();
    }
    Ok(result)
}

impl ProvisioningSession {
    pub async fn start(
        access: &NodeAccess,
        host: &HostPublicKey,
        root: &Path,
        identity_file: &Path,
        packet: ProvisionPacket,
    ) -> Result<Self> {
        if !host.matches_access(access)
            || packet.instance != access.instance_id()
            || !access.matches_image_sha256(&packet.image)
        {
            return Err(Error::Invalid);
        }
        let directory = private_directory(root)?;
        if !literal_path(identity_file)
            || identity_file.parent() != Some(root)
            || identity_file.canonicalize().map_err(|_| Error::Invalid)? != identity_file
        {
            return Err(Error::Invalid);
        }
        let identity = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(identity_file)
            .map_err(|_| Error::Invalid)?;
        private_file(&identity)?;
        let known = root.join("provision-known-hosts");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&known)
            .map_err(|_| Error::Invalid)?;
        private_file(&file)?;
        let address = access.ssh_address();
        writeln!(
            file,
            "[{}]:{} {}",
            address.ip(),
            address.port(),
            host.as_openssh()
        )
        .map_err(|_| Error::Unavailable)?;
        file.sync_all().map_err(|_| Error::Unavailable)?;
        directory.sync_all().map_err(|_| Error::Unavailable)?;
        let mut command = ssh_command(root, identity_file, &known, address);
        // Only these internally constructed packet types can select a remote command.
        if packet.command == "join-v0" {
            command = token_ssh_command(root, identity_file, &known, address);
        }
        let mut child = command.spawn().map_err(|_| Error::Unavailable)?;
        if let Some(mut stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut line = Vec::with_capacity(256);
                let mut block = [0; 512];
                let mut overflow = false;
                let mut seen = std::collections::BTreeSet::new();
                while let Ok(n) = stderr.read(&mut block).await {
                    if n == 0 {
                        break;
                    }
                    for b in &block[..n] {
                        if *b == b'\n' {
                            if !overflow
                                && let Some(label) = native_label(&line)
                                && seen.insert(label)
                            {
                                eprintln!("prototype native category: {label}");
                            }
                            line.fill(0);
                            line.clear();
                            overflow = false;
                        } else if line.len() < 256 && !overflow {
                            line.push(*b);
                        } else {
                            line.fill(0);
                            line.clear();
                            overflow = true;
                        }
                    }
                    block.fill(0);
                }
            });
        }
        let mut session = Self { child };
        let mut input = session.child.stdin.take().ok_or(Error::Unavailable)?;
        tokio::time::timeout(Duration::from_secs(20), async {
            input
                .write_all(packet.frame.expose())
                .await
                .map_err(|_| Error::Unavailable)?;
            input.shutdown().await.map_err(|_| Error::Unavailable)
        })
        .await
        .map_err(|_| Error::Unavailable)??;
        Ok(session)
    }
    /// Wait for the node to validate its inputs/instance credential and bind
    /// its control endpoint before dialing it. No plaintext bootstrap data is
    /// returned here: this is only a fixed progress protocol inside pinned SSH.
    pub async fn wait_listening(&mut self) -> std::result::Result<(), &'static str> {
        let output = self
            .child
            .stdout
            .take()
            .ok_or("prototype-provision-output-unavailable")?;
        wait_for_stages(output).await
    }
    /// Public node identity observed inside pinned SSH; not an owner certificate.
    pub async fn token_identity(&mut self, timeout: Duration) -> Result<Value> {
        let mut output = self.child.stdout.take().ok_or(Error::Unavailable)?;
        let bytes = tokio::time::timeout(timeout, async {
            let mut bytes = Vec::new();
            loop {
                let byte = output.read_u8().await.map_err(|_| Error::Unavailable)?;
                if byte == b'\n' {
                    break;
                }
                if bytes.len() >= 512 {
                    return Err(Error::Invalid);
                }
                bytes.push(byte);
            }
            Ok(bytes)
        })
        .await
        .map_err(|_| Error::Unavailable)??;
        serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)
    }
    pub async fn exited(&mut self) -> Result<()> {
        self.child
            .wait()
            .await
            .map(|_| ())
            .map_err(|_| Error::Unavailable)
    }
    pub async fn stop(&mut self) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(3), self.child.kill())
            .await
            .map_err(|_| Error::Unavailable)?
            .map_err(|_| Error::Unavailable)
    }
}

async fn wait_for_stages(
    mut output: impl tokio::io::AsyncRead + Unpin,
) -> std::result::Result<(), &'static str> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    for &(expected, failure) in crate::vast::provision_protocol::STAGES {
        let mut bytes = vec![0; expected.len()];
        tokio::time::timeout_at(deadline, output.read_exact(&mut bytes))
            .await
            .map_err(|_| failure)?
            .map_err(|_| failure)?;
        if bytes != expected {
            return Err(failure);
        }
    }
    Ok(())
}

fn ssh_command(
    root: &Path,
    identity: &Path,
    known: &Path,
    address: std::net::SocketAddr,
) -> Command {
    command_for("provision-v1", root, identity, known, address)
}
fn token_ssh_command(
    root: &Path,
    identity: &Path,
    known: &Path,
    address: std::net::SocketAddr,
) -> Command {
    command_for("join-v0", root, identity, known, address)
}
fn command_for(
    remote_command: &str,
    root: &Path,
    identity: &Path,
    known: &Path,
    address: std::net::SocketAddr,
) -> Command {
    let mut command = Command::new("/usr/bin/ssh");
    command
        .env_clear()
        .env("HOME", root)
        .env("PATH", "/usr/bin:/bin")
        .args([
            "-F",
            "/dev/null",
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "IdentityAgent=none",
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "GlobalKnownHostsFile=/dev/null",
            "-o",
            "PasswordAuthentication=no",
            "-o",
            "KbdInteractiveAuthentication=no",
            "-o",
            "PreferredAuthentications=publickey",
            "-o",
            "ConnectionAttempts=1",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=5",
            "-o",
            "ServerAliveCountMax=2",
            "-o",
            "ControlMaster=no",
            "-o",
            "ControlPath=none",
            "-o",
            "ClearAllForwardings=yes",
        ])
        .arg("-o")
        .arg(format!("UserKnownHostsFile={}", known.display()))
        .arg("-i")
        .arg(identity)
        .arg("-p")
        .arg(address.port().to_string())
        .arg(format!("m8s@{}", address.ip()))
        .arg(remote_command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}
fn private_directory(path: &Path) -> Result<File> {
    // OpenSSH parses the -o known-hosts value itself. Keep this generated run
    // path literal, with no config whitespace, expansion, quote or token syntax.
    if !literal_path(path)
        || !path.is_absolute()
        || path.canonicalize().map_err(|_| Error::Invalid)? != path
    {
        return Err(Error::Invalid);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(path)
        .map_err(|_| Error::Invalid)?;
    let meta = file.metadata().map_err(|_| Error::Invalid)?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o7777 != 0o700 {
        return Err(Error::Invalid);
    }
    m8s_secrets::check_no_extended_acl(&file).map_err(|_| Error::Invalid)?;
    Ok(file)
}
fn literal_path(path: &Path) -> bool {
    path.to_str().is_some_and(|text| {
        text.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    })
}
fn private_file(file: &File) -> Result<()> {
    let meta = file.metadata().map_err(|_| Error::Invalid)?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o7777 != 0o600
        || meta.nlink() != 1
    {
        return Err(Error::Invalid);
    }
    m8s_secrets::check_no_extended_acl(file).map_err(|_| Error::Invalid)
}

fn native_label(line: &[u8]) -> Option<&'static str> {
    let label = line.strip_prefix(b"m8s native diagnostic: ")?;
    [
        "native-argument-error",
        "native-certificate-error",
        "native-cuda-error",
        "native-runtime-error",
        "native-model-reference-error",
        "native-download-error",
        "native-other-error",
        "native-child-exited",
        "node-policy-error",
        "node-id-error",
        "node-material-error",
        "node-certificate-error",
    ]
    .into_iter()
    .find(|v| label == v.as_bytes())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod readiness_tests {
    use super::*;
    #[test]
    fn exact_ordered_progress_required_without_exposing_received_bytes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let all: Vec<u8> = crate::vast::provision_protocol::STAGES
                .iter()
                .flat_map(|(b, _)| b.iter().copied())
                .collect();
            assert!(wait_for_stages(all.as_slice()).await.is_ok());
            let stages = crate::vast::provision_protocol::STAGES;
            let mut prefix = Vec::new();
            for &(record, failure) in stages {
                assert_eq!(wait_for_stages(prefix.as_slice()).await, Err(failure));
                let mut wrong = prefix.clone();
                wrong.extend_from_slice(&vec![b'X'; record.len()]);
                assert_eq!(wait_for_stages(wrong.as_slice()).await, Err(failure));
                prefix.extend_from_slice(record);
            }
        });
    }
}

#[cfg(test)]
mod token_packet_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn token_mode_is_separate_and_bounded() {
        let config = json!({"schema":"m8s.token-join/v0","instance_id":123,"image_sha256":"a".repeat(64),"mesh_id":"b".repeat(16),"model":QWEN38_MODEL_REF,"lifetime_seconds":60});
        let packet =
            ProvisionPacket::token_join(&config, SecretValue::new(b"synthetic-token".to_vec()))
                .unwrap();
        assert_eq!(packet.command, "join-v0");
        assert!(packet.frame.expose().starts_with(b"M8S-TOKEN-JOIN-V0\n"));
        for token in [b"".as_slice(), b"new\nline", b"space token"] {
            assert!(
                ProvisionPacket::token_join(&config, SecretValue::new(token.to_vec())).is_err()
            );
        }
        let mut invalid = config.clone();
        invalid["lifetime_seconds"] = json!(1501);
        assert!(
            ProvisionPacket::token_join(&invalid, SecretValue::new(b"synthetic".to_vec())).is_err()
        );
        invalid = config;
        invalid["disable_auth"] = json!(true);
        assert!(
            ProvisionPacket::token_join(&invalid, SecretValue::new(b"synthetic".to_vec())).is_err()
        );
    }
}
