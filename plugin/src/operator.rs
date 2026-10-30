//! Shared v0 operation boundary used by Mesh MCP and the desktop's local adapter.
//! Requests cannot select executables, credential paths, tokens, or launch policy.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Offers {},
    Quote { offer_id: u64, machine_id: u64 },
    Launch { quote_id: String },
    Status {},
    Remove {},
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
// Configuration presence only; provider acceptance is checked by live operations.
// Project a boolean without reading or returning the key or its location.
fn provider_configured(plan: &Value) -> bool {
    let Some(path) = plan["credential_file"].as_str().map(Path::new) else {
        return false;
    };
    path.is_absolute()
        && fs::symlink_metadata(path).is_ok_and(|m| {
            m.is_file()
                && m.uid() == unsafe { libc::geteuid() }
                && m.mode() & 0o022 == 0
                && m.len() <= 65536
                && m.nlink() == 1
        })
}
fn read(path: &Path) -> Result<Value> {
    ensure!(
        path.is_absolute() && path.canonicalize()? == path,
        "Invalid private input"
    );
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = file.metadata()?;
    ensure!(
        m.is_file()
            && m.uid() == unsafe { libc::geteuid() }
            && m.mode() & 0o077 == 0
            && m.len() <= 32768,
        "Invalid private input"
    );
    let mut bytes = Vec::new();
    file.take(32769).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 32768, "Oversized input");
    Ok(serde_json::from_slice(&bytes)?)
}
fn write_new(path: &Path, value: &Value) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(&serde_json::to_vec(value)?)?;
    f.sync_all()?;
    fs::File::open(path.parent().context("Missing parent")?)?.sync_all()?;
    Ok(())
}
struct Config {
    cli: PathBuf,
    plan: Value,
    root: PathBuf,
}
impl Config {
    fn load() -> Result<Self> {
        let cli = PathBuf::from(std::env::var_os("M8S_CLI").context("Configure M8S_CLI")?);
        let m = fs::symlink_metadata(&cli)?;
        ensure!(
            cli.is_absolute()
                && cli.canonicalize()? == cli
                && m.is_file()
                && m.uid() == unsafe { libc::geteuid() }
                && m.mode() & 0o7022 == 0
                && m.mode() & 0o111 != 0,
            "Invalid controller"
        );
        let path = PathBuf::from(std::env::var_os("M8S_V0_PLAN").context("Configure M8S_V0_PLAN")?);
        let plan = read(&path)?;
        ensure!(
            plan["schema"] == "m8s.vast-demo/v1"
                && plan["model"] == "qwen3.8-27b"
                && plan["token_target"].is_string()
                && plan.get("attachment").is_none(),
            "Expected explicit token-only plan"
        );
        let root = PathBuf::from(
            plan["control_directory"]
                .as_str()
                .context("Missing control directory")?,
        );
        let m = fs::symlink_metadata(&root)?;
        ensure!(
            root.is_absolute()
                && root.canonicalize()? == root
                && m.is_dir()
                && m.uid() == unsafe { libc::geteuid() }
                && m.mode() & 0o777 == 0o700,
            "Invalid control directory"
        );
        Ok(Self { cli, plan, root })
    }
    fn target(&self) -> Result<Value> {
        read(Path::new(
            self.plan["token_target"]
                .as_str()
                .context("Missing target")?,
        ))
    }
    fn cap(&self) -> Result<u64> {
        let p = &self.plan["policy"];
        let cap = p["trial_all_in"]
            .as_u64()
            .and_then(|v| v.checked_sub(p["baseline"]["prior_lifetime_spent"].as_u64()?))
            .context("Invalid allowance")?;
        ensure!(
            (1..=6_000_000).contains(&cap),
            "Allowance exceeds v0 ceiling"
        );
        Ok(cap)
    }
    async fn cli(&self, args: Vec<String>) -> Result<Value> {
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            tokio::process::Command::new(&self.cli)
                .args(args)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .stdout(Stdio::piped())
                .kill_on_drop(true)
                .output(),
        )
        .await??;
        ensure!(
            result.status.success() && result.stdout.len() <= 131072,
            "Controller operation refused"
        );
        Ok(serde_json::from_slice(&result.stdout)?)
    }
    fn command_args(&self, operation: &str) -> Result<Vec<String>> {
        Ok(vec![
            "prototype".into(),
            operation.into(),
            "--credential-file".into(),
            self.plan["credential_file"]
                .as_str()
                .context("Missing credential selection")?
                .into(),
        ])
    }
}
async fn verify_target(target: &Value) -> Result<Value> {
    let port = target["console_port"]
        .as_u64()
        .filter(|p| *p > 0 && *p <= 65535)
        .context("Invalid local port")?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()?;
    let mut response = client
        .get(format!("http://127.0.0.1:{port}/api/status"))
        .send()
        .await?;
    ensure!(response.status().is_success(), "Target unavailable");
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(bytes.len() + chunk.len() <= 65536, "Oversized status");
        bytes.extend_from_slice(&chunk);
    }
    let status: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        status["node_id"] == target["node_id"] && status["mesh_id"] == target["mesh_id"],
        "Selected mesh changed"
    );
    Ok(status)
}
// Read-only booking estimate. The controller independently performs durable
// budget admission and fresh provider checks before CREATE.
fn quote_estimate(plan: &Value, prices: &Value, at: i64) -> anyhow::Result<u64> {
    let get = |value: &Value| {
        value
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing budget amount"))
    };
    let policy = &plan["policy"];
    let quote = &plan["request"]["quote"];
    let runtime = get(&quote["runtime_seconds"])?;
    let cleanup = get(&quote["cleanup_seconds"])?;
    let seconds = runtime
        .checked_add(cleanup)
        .ok_or_else(|| anyhow::anyhow!("Invalid duration"))?;
    anyhow::ensure!(
        runtime > 0
            && runtime <= get(&policy["max_runtime_seconds"])?
            && seconds <= 1800
            && cleanup >= get(&policy["minimum_cleanup_seconds"])?
            && cleanup >= 240,
        "Invalid duration"
    );
    let end = at
        .checked_add(seconds as i64)
        .ok_or_else(|| anyhow::anyhow!("Invalid deadline"))?;
    anyhow::ensure!(
        policy["valid_from"].as_i64().is_some_and(|v| v <= at)
            && policy["valid_until"].as_i64().is_some_and(|v| v >= end),
        "Approval expired"
    );
    let contingency = get(&quote["contingency"])?;
    anyhow::ensure!(
        contingency >= get(&policy["minimum_contingency"])? && contingency > 0,
        "Invalid reserve"
    );
    let compute = (u128::from(get(&prices["compute_micro_usd_per_hour"])?) * u128::from(seconds))
        .div_ceil(3600);
    let storage = (u128::from(get(&prices["storage_micro_usd_per_hour"])?) * u128::from(seconds))
        .div_ceil(3600);
    let total = compute
        + storage
        + u128::from(get(&prices["transfer_micro_usd_per_gb"])?) * 64
        + u128::from(contingency);
    let hourly = (total * 3600).div_ceil(u128::from(runtime));
    let baseline = &policy["baseline"];
    anyhow::ensure!(
        total + u128::from(get(&baseline["prior_lifetime_spent"])?)
            <= u128::from(get(&policy["trial_all_in"])?)
            && total + u128::from(get(&baseline["prior_current_period_spent"])?)
                <= u128::from(get(&policy["monthly_all_in"])?)
            && hourly <= u128::from(get(&policy["instance_hourly"])?)
            && hourly <= u128::from(get(&policy["total_hourly"])?),
        "Budget does not cover the full reserve"
    );
    Ok(u64::try_from(total)?)
}

fn worker_present(status: &Value, ready: &Value) -> bool {
    status["is_host"] == false
        && status["peers"].as_array().is_some_and(|peers| {
            peers.len() == 1
                && ready["node_id"].as_str().is_some_and(|id| {
                    peers[0]["id"].as_str().or(peers[0]["node_id"].as_str()) == Some(id)
                })
        })
}
fn worker_observation(
    value: Option<&Value>,
    node: Option<&Value>,
    plan: &Value,
    at: u64,
) -> Option<Value> {
    let (value, node) = (value?, node?);
    let instance = value["instance_id"].as_u64().filter(|id| *id > 0)?;
    if value["schema"] != "m8s.token-observation/v0"
        || value["image"] != plan["image"]
        || node["instance_id"].as_u64() != Some(instance)
        || value["observed_at"]
            .as_u64()
            .and_then(|n| at.checked_sub(n))
            .is_none_or(|age| age > 30)
    {
        return None;
    }
    let worker = &value["worker"];
    let phase = worker["phase"].as_str().filter(|p| {
        matches!(
            *p,
            "awaiting_join" | "unknown" | "starting" | "waiting_for_mesh" | "joined" | "stopped"
        )
    })?;
    if worker["schema"] != "m8s.token-worker-status/v0" {
        return None;
    }
    let mut result = json!({"phase":phase,"observed_at":value["observed_at"]});
    for key in [
        "elapsed_seconds",
        "cache_file_bytes",
        "cache_allocated_bytes",
        "cache_file_count",
        "cache_incomplete_files",
        "worker_rss_bytes",
        "worker_read_bytes",
        "worker_write_bytes",
        "network_received_bytes",
        "network_sent_bytes",
    ] {
        if let Some(n) = worker[key].as_u64() {
            result[key] = n.into();
        }
    }
    for key in [
        "worker_running",
        "api_reachable",
        "mesh_matches",
        "cache_scan_truncated",
    ] {
        if let Some(v) = worker[key].as_bool() {
            result[key] = v.into();
        }
    }
    Some(result)
}
pub async fn execute(request: Request) -> Result<Value> {
    let c = Config::load()?;
    execute_config(&c, request).await
}
async fn execute_config(c: &Config, request: Request) -> Result<Value> {
    // Read-only observations must remain available while quotes or launches mutate
    // control state. Partial/new files project conservatively as not ready.
    let _lock = if matches!(&request, Request::Status {} | Request::Offers {}) {
        None
    } else {
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(c.root.join("operator.lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock).context("Another operation is active")?;
        Some(lock)
    };
    let intent = c.root.join("operator-intent.json");
    let result = match request {
        Request::Offers {} => {
            let mut args = c.command_args("vast-offers")?;
            args.extend(["--disk-gb".into(), "80".into()]);
            c.cli(args).await?
        }
        Request::Quote {
            offer_id,
            machine_id,
        } => {
            ensure!(
                !intent.try_exists()? && offer_id > 0 && machine_id > 0,
                "Attempt already consumed or invalid offer"
            );
            let target = c.target()?;
            verify_target(&target).await?;
            let mut args = c.command_args("vast-quote")?;
            args.extend([
                "--account".into(),
                c.plan["expected_account_id"]
                    .as_u64()
                    .context("Missing account")?
                    .to_string(),
                "--disk-gb".into(),
                "80".into(),
                "--offer".into(),
                offer_id.to_string(),
                "--machine".into(),
                machine_id.to_string(),
            ]);
            let prices = c.cli(args).await?;
            ensure!(prices["offer_id"] == offer_id, "Offer changed");
            let estimate = quote_estimate(&c.plan, &prices, i64::try_from(now())?)?;
            let mut nonce = [0u8; 32];
            getrandom::fill(&mut nonce).map_err(|_| anyhow::anyhow!("Randomness unavailable"))?;
            let id = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
            let mut plan = c.plan.clone();
            plan["offer_id"] = json!(offer_id);
            plan["offer_machine_id"] = json!(machine_id);
            let quote = json!({"quote_id":id,"expires_at":now()+30,"plan":plan,"target":target,"prices":prices});
            write_new(&c.root.join(format!("quote-{id}.json")), &quote)?;
            json!({"quote_id":id,"expires_at":quote["expires_at"],"model":"Qwen 3.8 27B · token-only v0","target":{"node_id":target["node_id"],"mesh_id":target["mesh_id"]},"cap_micro_usd":c.cap()?,"estimated_reserve_micro_usd":estimate,"prices":prices,"admission":"token-only-provider-trusted","certificate_verified":false})
        }
        Request::Launch { quote_id } => {
            ensure!(
                !intent.try_exists()?
                    && quote_id.len() == 64
                    && quote_id.bytes().all(|b| b.is_ascii_hexdigit()),
                "Attempt consumed or invalid quote"
            );
            let quote = read(&c.root.join(format!("quote-{quote_id}.json")))?;
            ensure!(
                quote["expires_at"].as_u64().is_some_and(|t| t >= now())
                    && quote["target"] == c.target()?,
                "Expired quote or changed target"
            );
            verify_target(&quote["target"]).await?;
            let plan = quote["plan"].clone();
            let mut expected = c.plan.clone();
            expected["offer_id"] = plan["offer_id"].clone();
            expected["offer_machine_id"] = plan["offer_machine_id"].clone();
            ensure!(plan == expected, "Changed policy or configuration");
            c.cap()?;
            let plan_path = c.root.join("operator-plan.json");
            write_new(&plan_path, &plan)?;
            write_new(
                &intent,
                &json!({"schema":"m8s.operation/v0","quote_id":quote_id,"created_at":now(),"expires_at":now()+1800,"offer_id":plan["offer_id"],"target_node":quote["target"]["node_id"],"target_mesh":quote["target"]["mesh_id"]}),
            )?;
            let log = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(c.root.join("operator.log"))?;
            let _child = std::process::Command::new(&c.cli)
                .args(["prototype", "launch", "--plan"])
                .arg(plan_path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(log)
                .spawn()?;
            json!({"accepted":true,"admission":"token-only-provider-trusted","certificate_verified":false})
        }
        Request::Remove {} => {
            ensure!(intent.try_exists()?, "No attempt");
            let stop = c.root.join("down-requested");
            if !stop.try_exists()? {
                write_new(&stop, &json!({"requested":true}))?;
            }
            json!({"removal_requested":true,"removed":false})
        }
        Request::Status {} => {
            let attempt = PathBuf::from(
                c.plan["directory"]
                    .as_str()
                    .context("Missing attempt directory")?,
            );
            let cleanup = read(&attempt.join("cleanup-evidence.json")).ok();
            let node = read(&attempt.join("desktop-node.json")).ok();
            let removed = cleanup.as_ref().zip(node.as_ref()).is_some_and(|(r, n)| {
                r["schema"] == "m8s.vast-removal/v1"
                    && r["provider_confirmed_absent"] == true
                    && r["image"] == c.plan["image"]
                    && r["creator_account_id"] == c.plan["expected_account_id"]
                    && r["instance_id"]
                        .as_u64()
                        .is_some_and(|id| id > 0 && n["instance_id"] == id)
            });
            let evidence = read(&attempt.join("demo-evidence.json")).ok();
            let ready = read(&attempt.join("token-ready.json")).ok();
            let observation = read(&attempt.join("token-observation.json")).ok();
            let worker_status = if removed {
                None
            } else {
                worker_observation(observation.as_ref(), node.as_ref(), &c.plan, now())
            };
            let target = c.target()?;
            let live = verify_target(&target).await.ok();
            let active_ready = !removed
                && !c.root.join("down-requested").exists()
                && ready.as_ref().zip(node.as_ref()).is_some_and(|(r, n)| {
                    r["schema"] == "m8s.token-ready/v0"
                        && r["certificate_verified"] == false
                        && r["instance_id"] == n["instance_id"]
                        && r["mesh_id"] == target["mesh_id"]
                        && live
                            .as_ref()
                            .is_some_and(|status| worker_present(status, r))
                });

            json!({"provider_configured":provider_configured(&c.plan),"worker_status":worker_status,"ready":active_ready,"offer_id":read(&intent).ok().map(|v|v["offer_id"].clone()),"expires_at":read(&intent).ok().map(|v|v["expires_at"].clone()),"removal_requested":c.root.join("down-requested").exists(),"node_id":ready.as_ref().and_then(|r|r["node_id"].as_str()),"target":{"node_id":target["node_id"],"mesh_id":target["mesh_id"]},"attempt_consumed":intent.try_exists()?,"provider_confirmed_removed":removed,"instance_id":node.as_ref().and_then(|v|v["instance_id"].as_u64()),"certificate_verified":false,"admission":"token-only-provider-trusted","inference_verified":removed&&evidence.as_ref().is_some_and(|e|e["schema"]=="m8s.vast-demo-evidence/v1"&&e["certificate_verified"]==false&&e["admission"]=="token-only-provider-trusted"&&e["owner_routed_inference"]==true&&e["provider_confirmed_absent"]==true&&e["image"]==c.plan["image"]&&node.as_ref().is_some_and(|n|e["instance_id"]==n["instance_id"]))})
        }
    };
    Ok(json!({"schema":"m8s.operator/v0","result":result}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observations_are_fresh_resource_bound_and_not_readiness() {
        let value = json!({"schema":"m8s.token-observation/v0","image":"image","instance_id":7,"observed_at":100,"worker":{"schema":"m8s.token-worker-status/v0","phase":"starting","worker_running":true,"cache_file_bytes":123,"join_token":"private-token","ready":true}});
        let node = json!({"instance_id":7});
        let plan = json!({"image":"image"});
        let observed = worker_observation(Some(&value), Some(&node), &plan, 110).unwrap();
        assert_eq!(observed["cache_file_bytes"], 123);
        assert!(observed.get("ready").is_none());
        assert!(!observed.to_string().contains("private"));
        for (field, changed) in [
            ("instance_id", json!(8)),
            ("image", json!("other")),
            ("observed_at", json!(79)),
            ("observed_at", json!(111)),
        ] {
            let mut bad = value.clone();
            bad[field] = changed;
            assert!(worker_observation(Some(&bad), Some(&node), &plan, 110).is_none());
        }
    }
    #[tokio::test]
    async fn status_remains_available_while_a_mutation_holds_the_lock() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let target = root.join("target.json");
        write_new(
            &target,
            &json!({"node_id":"node","mesh_id":"mesh","console_port":0}),
        )
        .unwrap();
        let credential = root.join("synthetic.env");
        write_new(&credential, &json!({"fixture":true})).unwrap();
        let c = Config {
            cli: root.join("unused-controller"),
            plan: json!({"directory":root.join("unused-attempt"),"token_target":target,"credential_file":credential}),
            root: root.clone(),
        };
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(root.join("operator.lock"))
            .unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        let status = execute_config(&c, Request::Status {}).await.unwrap();
        assert_eq!(status["result"]["provider_configured"], true);
        assert_eq!(status["result"]["attempt_consumed"], false);
        assert_eq!(status["result"]["ready"], false);
        let mutation = execute_config(
            &c,
            Request::Launch {
                quote_id: "synthetic".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(mutation.to_string().contains("Another operation is active"));
        assert!(!root.join("operator-intent.json").exists());
    }
    #[test]
    fn provider_configuration_refuses_missing_shared_or_linked_files() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("selected.env");
        let plan = json!({"credential_file":file});
        assert!(!provider_configured(&plan));
        fs::write(&file, "synthetic fixture; presence is not key verification").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(provider_configured(&plan));
        fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(!provider_configured(&plan));
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        let link = temp.path().join("linked.env");
        symlink(&file, &link).unwrap();
        assert!(!provider_configured(&json!({"credential_file":link})));
        fs::remove_file(&link).unwrap();
        fs::hard_link(&file, &link).unwrap();
        assert!(!provider_configured(&plan));
        assert!(!provider_configured(
            &json!({"credential_file":"relative.env"})
        ));
    }
    #[test]
    fn readiness_requires_the_live_sole_worker() {
        let ready = json!({"node_id":"worker"});
        assert!(worker_present(
            &json!({"is_host":false,"peers":[{"id":"worker"}]}),
            &ready
        ));
        for status in [
            json!({"is_host":false,"peers":[]}),
            json!({"is_host":false,"peers":[{"id":"other"}]}),
            json!({"is_host":true,"peers":[{"id":"worker"}]}),
            json!({"is_host":false,"peers":[{"id":"worker"},{"id":"other"}]}),
        ] {
            assert!(!worker_present(&status, &ready));
        }
    }
    #[test]
    fn requests_cannot_override_private_configuration() {
        for v in [
            json!({"operation":"launch","quote_id":"x","plan":"/tmp/other"}),
            json!({"operation":"offers","api_key":"secret"}),
            json!({"operation":"shell","command":"anything"}),
        ] {
            assert!(serde_json::from_value::<Request>(v).is_err());
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::{
        net::TcpListener,
        os::unix::fs::PermissionsExt,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    fn budget_plan(at: i64) -> Value {
        json!({"policy":{"trial_all_in":6000010,"monthly_all_in":6000010,"baseline":{"prior_lifetime_spent":10,"prior_current_period_spent":10},"max_runtime_seconds":1500,"minimum_cleanup_seconds":240,"valid_from":at-1,"valid_until":at+1800,"minimum_contingency":1000000,"instance_hourly":18000000,"total_hourly":18000000},"request":{"quote":{"runtime_seconds":1500,"cleanup_seconds":240,"contingency":1000000}}})
    }
    #[test]
    fn quote_covers_transfer_cleanup_and_contingency_before_booking() {
        let prices = json!({"compute_micro_usd_per_hour":1800000,"storage_micro_usd_per_hour":50000,"transfer_micro_usd_per_gb":4000});
        let plan = budget_plan(1000);
        assert_eq!(quote_estimate(&plan, &prices, 1000).unwrap(), 2_150_167);
        for field in [
            "trial_all_in",
            "monthly_all_in",
            "instance_hourly",
            "total_hourly",
        ] {
            let mut short = plan.clone();
            short["policy"][field] = json!(1_000_000);
            assert!(quote_estimate(&short, &prices, 1000).is_err(), "{field}");
        }
        let mut expensive = prices.clone();
        expensive["transfer_micro_usd_per_gb"] = json!(100_000);
        assert!(quote_estimate(&plan, &expensive, 1000).is_err());
        let mut incomplete = prices.clone();
        incomplete
            .as_object_mut()
            .unwrap()
            .remove("transfer_micro_usd_per_gb");
        assert!(quote_estimate(&plan, &incomplete, 1000).is_err());
        assert!(quote_estimate(&plan, &prices, 2000).is_err());
        let mut no_cleanup = plan.clone();
        no_cleanup["request"]["quote"]["cleanup_seconds"] = json!(239);
        assert!(quote_estimate(&no_cleanup, &prices, 1000).is_err());
    }
    #[tokio::test]
    async fn quote_then_one_launch_retains_intent_and_requires_exact_cleanup() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let server = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                if let Ok((mut socket, _)) = listener.accept() {
                    socket.set_nonblocking(false).unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = Vec::new();
                    while request.len() < 8192 && !request.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        if socket.read(&mut byte).unwrap_or(0) != 1 {
                            break;
                        }
                        request.push(byte[0]);
                    }
                    let body = r#"{"node_id":"node","mesh_id":"mesh"}"#;
                    let _ = write!(
                        socket,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });
        let target = root.join("target.json");
        write_new(
            &target,
            &json!({"node_id":"node","mesh_id":"mesh","console_port":port}),
        )
        .unwrap();
        let cli = root.join("controller");
        fs::write(&cli,format!("#!/bin/sh\nif [ \"$2\" = vast-quote ]; then printf '%s' '{{\"offer_id\":7,\"compute_micro_usd_per_hour\":1800000,\"storage_micro_usd_per_hour\":50000,\"transfer_micro_usd_per_gb\":4000}}'; exit 0; fi\nif [ \"$2\" = launch ]; then test -f '{}/operator-intent.json' || exit 1; printf 'invoked' > '{}/invoked'; exit 0; fi\nexit 1\n",root.display(),root.display())).unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
        let attempt = root.join("attempt");
        fs::create_dir(&attempt).unwrap();
        let mut plan = budget_plan(now() as i64);
        for (key, value) in json!({"control_directory":root,"directory":attempt,"token_target":target,"credential_file":"/synthetic/key.env","expected_account_id":42,"image":"test-image"}).as_object().unwrap() { plan[key] = value.clone(); }
        let c = Config {
            cli,
            root: root.clone(),
            plan,
        };
        let mut underfunded = Config {
            cli: c.cli.clone(),
            plan: c.plan.clone(),
            root: c.root.clone(),
        };
        underfunded.plan["policy"]["trial_all_in"] = json!(1_000_010);
        assert!(
            execute_config(
                &underfunded,
                Request::Quote {
                    offer_id: 7,
                    machine_id: 9
                }
            )
            .await
            .is_err()
        );
        assert!(!root.join("operator-intent.json").exists());
        assert!(!fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("quote-")
        }));
        let quote = execute_config(
            &c,
            Request::Quote {
                offer_id: 7,
                machine_id: 9,
            },
        )
        .await
        .unwrap();
        assert_eq!(quote["result"]["estimated_reserve_micro_usd"], 2_150_167);
        let id = quote["result"]["quote_id"].as_str().unwrap().to_owned();
        assert!(
            execute_config(
                &c,
                Request::Launch {
                    quote_id: id.clone()
                }
            )
            .await
            .unwrap()["result"]["accepted"]
                == true
        );
        assert!(
            execute_config(&c, Request::Launch { quote_id: id })
                .await
                .is_err()
        );
        for _ in 0..100 {
            if root.join("invoked").exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(root.join("invoked").exists());
        execute_config(&c, Request::Remove {}).await.unwrap();
        assert!(root.join("down-requested").exists());
        assert!(root.join("operator-intent.json").exists());
        write_new(
            &attempt.join("desktop-node.json"),
            &json!({"instance_id":123}),
        )
        .unwrap();
        write_new(&attempt.join("cleanup-evidence.json"),&json!({"schema":"m8s.vast-removal/v1","provider_confirmed_absent":true,"instance_id":999,"image":"test-image","creator_account_id":42})).unwrap();
        let status = execute_config(&c, Request::Status {}).await.unwrap();
        assert_eq!(status["result"]["provider_confirmed_removed"], false);
        let path = attempt.join("cleanup-evidence.json");
        fs::remove_file(&path).unwrap();
        write_new(&path,&json!({"schema":"m8s.vast-removal/v1","provider_confirmed_absent":true,"instance_id":123,"image":"test-image","creator_account_id":42})).unwrap();
        let status = execute_config(&c, Request::Status {}).await.unwrap();
        assert_eq!(status["result"]["provider_confirmed_removed"], true);
        assert_eq!(status["result"]["certificate_verified"], false);
        assert!(!status.to_string().contains("/synthetic"));
        stop.store(true, Ordering::Relaxed);
        server.join().unwrap();
    }
}
