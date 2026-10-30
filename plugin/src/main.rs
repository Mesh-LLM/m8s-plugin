mod operator;
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
            mcp::tool("compute").input::<operator::Request>()
                .description("m8s.operator/v0: offers, quote, launch, status or remove for the explicitly configured token-only rental. Never accepts credentials or policy overrides. Launch requires a fresh quote_id; readiness is not certificate verification.")
                .handle(|args, _context| Box::pin(async {
                    let request = args;
                    Ok(operator::execute(request).await.map_err(|_|anyhow::anyhow!("Compute operation refused; check selected local configuration and retained attempt evidence"))?)
                })),
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
        Some("--request") => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::io::stdin().take(8193).read_to_end(&mut bytes)?;
            if bytes.len() > 8192 {
                bail!("Request too large");
            }
            let request = serde_json::from_slice(&bytes).context("Invalid v0 request")?;
            let value = operator::execute(request)
                .await
                .map_err(|_| anyhow::anyhow!("Compute operation refused"))?;
            println!("{}", serde_json::to_string(&value)?);
            Ok(())
        }
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
