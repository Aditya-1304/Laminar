use anyhow::Result;

fn main() -> Result<()> {
    let _ = laminar_config::AppConfig::from_env()?;
    laminar_telemetry::init_tracing("indexer")?;
    tracing::info!("laminar indexer scaffold booted");
    Ok(())
}
