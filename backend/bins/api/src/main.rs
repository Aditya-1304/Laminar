use anyhow::Result;

fn main() -> Result<()> {
    let _ = laminar_config::AppConfig::from_env()?;
    laminar_telemetry::init_tracing("api")?;
    tracing::info!("laminar api scaffold booted");
    Ok(())
}
