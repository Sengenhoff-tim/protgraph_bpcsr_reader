use std::fs::File;
use anyhow::{Context, Result};
use tracing_subscriber::Layer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub file_path: String,
    pub show_target: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file_path: "exceptions.log".to_string(),
            show_target: false,
        }
    }
}

pub fn init_logging(config: LoggingConfig) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_file = File::create(&config.file_path)
        .with_context(|| format!("failed to create log file '{}'", config.file_path))?;
    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);
    
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(config.show_target);

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(config.show_target)
        .with_filter(tracing_subscriber::filter::LevelFilter::WARN);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .context("failed to initialize tracing subscriber")?;

    Ok(guard)
}