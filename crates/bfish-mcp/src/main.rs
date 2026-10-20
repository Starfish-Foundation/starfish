use std::time::Duration;

use anyhow::Result;
use bfish_mcp::{install_plugin, Engine, DEFAULT_STUDIO_PORT};

fn print_help() {
    println!(
        r#"BFish MCP — Roblox Studio tools for any LLM

USAGE:
    bfish-mcp [OPTIONS]

OPTIONS:
    --stdio                 Speak MCP over stdin/stdout (default).
    --http                  Serve MCP at http://127.0.0.1:<port>/mcp.
    --port <PORT>           Studio bridge / HTTP port (default {DEFAULT_STUDIO_PORT}).
    --install-plugin        Install the Studio connector and exit.
    --no-bridge             Do not start the Studio bridge (stdio mode only).
    -h, --help              Show this help.
    -V, --version           Print the version.

The Studio bridge is always bound to 127.0.0.1 and is never exposed to the network.
Plugins: https://github.com/bfish-tools/bfish
"#
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bfish_mcp=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let mut args = std::env::args().skip(1);
    let mut http = false;
    let mut no_bridge = false;
    let mut install = false;
    let mut port = DEFAULT_STUDIO_PORT;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stdio" => http = false,
            "--http" => http = true,
            "--no-bridge" => no_bridge = true,
            "--install-plugin" => install = true,
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--port requires a number"))?;
                port = value.parse()?;
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("bfish-mcp {}", bfish_core::APP_VERSION);
                return Ok(());
            }
            other => {
                eprintln!("Unknown option: {other}\n");
                print_help();
                std::process::exit(2);
            }
        }
    }

    if install {
        let path = install_plugin()?;
        println!("BFish Connector installed:\n  {}", path.display());
        println!("\nRestart Roblox Studio, then open BFish Connector from the Plugins tab.");
        if !http && std::env::args().count() <= 2 {
            return Ok(());
        }
    }

    let mut engine = Engine::new(port);

    if http {
        println!("BFish MCP listening on http://127.0.0.1:{port}/mcp (Studio bridge: /studio)");
        engine.bridge().serve(engine.clone()).await?;
        return Ok(());
    }

    if !no_bridge {
        match engine.bridge().bind().await {
            Ok(listener) => {
                let bridge = engine.bridge().clone();
                let bridge_engine = engine.clone();
                tokio::spawn(async move {
                    if let Err(err) = bridge.serve_on(listener, bridge_engine).await {
                        tracing::error!("Studio bridge stopped: {err:#}");
                    }
                });
            }
            Err(err) => {
                if bfish_mcp::existing_bridge(port).await {
                    tracing::warn!(
                        "Port {port} is owned by another BFish MCP server; using it as a remote bridge."
                    );
                    engine = Engine::remote(port);
                } else {
                    tracing::error!("Studio bridge failed on port {port}: {err:#}");
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    bfish_mcp::mcp::serve_stdio(engine).await
}
