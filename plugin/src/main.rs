use anyhow::{Context, Result, bail};
use mesh_llm_plugin::{
    Plugin, PluginMetadata, PluginRuntime, SimplePlugin, mcp, package_manifest_json, plugin,
    plugin_server_info,
};
use std::{path::PathBuf, time::Duration};

fn definition() -> SimplePlugin {
    plugin! {
        metadata: PluginMetadata::new(
            "m8s", "0.1.0",
            plugin_server_info("m8s", "0.1.0", "m8s Compute", "Inspect your rented compute controller", None::<String>),
        ),
        mcp: [
            mcp::tool("status")
                .description("Read the local m8s controller's reported fleet state. Does not rent compute or establish live readiness.")
                .handle(|_args, _context| Box::pin(async {
                    let path = std::env::var_os("M8S_CONTROL_SOCKET")
                        .map(PathBuf::from)
                        .context("Set M8S_CONTROL_SOCKET to the owner controller socket")?;
                    let snapshot = tokio::task::spawn_blocking(move || {
                        let client = m8s_cli::dashboard::client::StatusClient::new(Duration::from_secs(3))
                            .map_err(|_| anyhow::anyhow!("Invalid observation timeout"))?;
                        client.status_with_config(&path)
                            .map_err(|_| anyhow::anyhow!("Controller unavailable or socket ownership verification failed"))
                    }).await.context("Controller observation task failed")??;
                    Ok(serde_json::to_value(snapshot).context("Status serialization failed")?)
                })),
        ],
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let plugin = definition();
    match std::env::args().nth(1).as_deref() {
        Some("--print-package-manifest") => {
            println!(
                "{}",
                package_manifest_json(&plugin.manifest().context("Plugin manifest unavailable")?)?
            );
            Ok(())
        }
        Some(_) => bail!("Unsupported argument"),
        None => PluginRuntime::run(plugin).await,
    }
}
