use anyhow::Result;

fn main() -> Result<()> {
    let _ = laminar_config::AppConfig::from_env()?;
    laminar_telemetry::init_tracing("keeper")?;
    tracing::info!("laminar keeper scaffold booted");
    Ok(())
}
