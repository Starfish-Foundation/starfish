use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bfish_backend=info,bfish_mcp=info,tower_http=warn".into()),
        )
        .init();

    let port = std::env::var("BFISH_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8765u16);

    bfish_backend::serve(port).await
}
