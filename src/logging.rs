use std::path::PathBuf;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    EnvFilter, layer::SubscriberExt, util::SubscriberInitExt,
};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use std::sync::Once;
use tracing::Level;

/// Initialize the application logger with both console and file outputs
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    // Create log directory if it doesn't exist
    let log_dir = get_log_directory();
    std::fs::create_dir_all(&log_dir)?;
    
    // Set up file appender with rotation
    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir,
        "mdesk.log",
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Store guard in a static to prevent it from being dropped
    // This ensures the logging continues for the lifetime of the application
    static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    unsafe {
        GUARD = Some(_guard);
    }
    
    // Initialize tracing subscriber with both console and file outputs
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| {
            // Default to INFO for the app, WARN for dependencies
            EnvFilter::builder()
                .parse("warn,m_desk_new=info")
        })?;
    
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt::layer() // Console layer
            .with_target(true)
            .with_file(true)
            .with_line_number(true))
        .with(fmt::layer() // File layer
            .with_writer(non_blocking)
            .with_ansi(false) // Disable ANSI colors in log files
            .with_target(true)
            .with_file(true)
            .with_line_number(true))
        .init();
    
    tracing::info!("Logging initialized");
    Ok(())
}

/// Initialize simple console-only logging for development
pub fn init_simple(level: Level) -> Result<(), Box<dyn std::error::Error>> {
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| {
            // Use provided level for app, WARN for dependencies
            let filter_str = format!("warn,m_desk_new={}", level.as_str().to_lowercase());
            EnvFilter::builder().parse(filter_str)
        })?;
        
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt::layer()
            .with_target(true)
            .with_file(true)
            .with_line_number(true))
        .init();
    
    tracing::info!("Simple logging initialized at level {}", level);
    Ok(())
}

/// Get the directory where log files will be stored
fn get_log_directory() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Failed to find home directory");
    home_dir.join(".mdesk").join("logs")
}

/// A helper function that logs with both eprintln and tracing during migration
/// This helps ensure logs are visible both with and without the tracing setup.
/// 
/// Usage:
///   log!(tracing::info, "My log message {}", value);
///   log!(tracing::error, "Error: {}", error);
/// 
/// Will be removed once migration is complete.
#[macro_export]
macro_rules! log {
    ($level:expr, $fmt:expr $(, $arg:expr)*) => {
        // Always print to stderr for backward compatibility
        eprintln!($fmt $(, $arg)*);
        // Also use tracing
        $level!($fmt $(, $arg)*);
    };
}

/// Function to check if tracing is properly initialized
pub fn is_tracing_initialized() -> bool {
    static TRACING_CHECK: Once = Once::new();
    static mut INITIALIZED: bool = false;
    
    TRACING_CHECK.call_once(|| {
        // Try to log something and see if it causes panic
        // If tracing is not initialized, nothing will happen
        // If tracing is initialized, the message will be logged
        let result = std::panic::catch_unwind(|| {
            tracing::trace!("Checking if tracing is initialized");
        });
        
        unsafe {
            INITIALIZED = result.is_ok();
        }
    });
    
    unsafe { INITIALIZED }
}
