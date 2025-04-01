# mDesk Logging System Documentation

This document explains how to use and configure logging in the mDesk application.

## Logging Overview

mDesk uses the `tracing` ecosystem for structured, level-based logging with the following features:

- Console output for development
- File output for production with daily log rotation
- Environment variable-based configuration
- Multiple log levels (TRACE, DEBUG, INFO, WARN, ERROR)
- Detailed logging context (file, line number, etc.)

## Log Levels

mDesk uses the following log levels (from most verbose to least verbose):

1. `TRACE`: Extremely detailed information, useful for debugging specific functions
2. `DEBUG`: Detailed information useful during development and debugging
3. `INFO`: General information about program execution (default level in development)
4. `WARN`: Potential issues that don't prevent proper execution
5. `ERROR`: Issues that prevent proper functionality

## Log File Location

Log files are stored in:

- **macOS/Linux**: `~/.mdesk/logs/`
- **Windows**: `C:\Users\[username]\.mdesk\logs\`

Files are rotated daily and named `mdesk.log-YYYY-MM-DD`.

## Configuring Log Levels

### Via Environment Variables

You can control log levels using the `RUST_LOG` environment variable:

```bash
# Set all modules to INFO, but mDesk code to DEBUG
RUST_LOG=info,m_desk_new=debug
```

### Via .env File

Create a `.env` file in the application directory:

```
RUST_LOG=info,m_desk_new=debug
```

### Common Configuration Examples

```
# Only show errors
RUST_LOG=error

# Default configuration (recommended for normal use)
RUST_LOG=warn,m_desk_new=info

# Development configuration (verbose)
RUST_LOG=warn,m_desk_new=debug

# Debugging with very detailed output
RUST_LOG=warn,m_desk_new=trace
```

## Adding Logging to Code

When modifying code, use the appropriate logging macros based on the event importance:

```rust
use tracing::{trace, debug, info, warn, error};

// Function execution - use trace or debug
trace!("Entering function with parameters: {:?}", params);

// Important state changes - use info
info!("Server connection established to {}", server.address);

// Recoverable issues - use warn
warn!("API request retry {} of {}", attempt, max_attempts);

// Problems requiring attention - use error
error!("Failed to connect to server: {}", error);
```

### Migration Helper

During the transition from `eprintln!` to structured logging, you can use the `log!` macro:

```rust
use crate::log;

// Logs to both eprintln and tracing
log!(tracing::info, "Server started on port {}", port);
```

## Viewing Logs

### Development

During development, logs are printed to the console with color-coding by level.

### Production

In production environments:

1. Check the log files in `~/.mdesk/logs/`
2. Use standard tools like `grep`, `tail`, or `less` to analyze logs

Example:
```bash
# View the latest log file
tail -f ~/.mdesk/logs/mdesk.log

# Search for errors
grep ERROR ~/.mdesk/logs/mdesk.log

# Follow logs in real-time filtering for errors
tail -f ~/.mdesk/logs/mdesk.log | grep ERROR
```

## JSON Logging

The logging system also supports JSON-formatted logs for machine parsing.
This is currently enabled for the log files but not for console output.

## Performance Considerations

- Use the appropriate log level to avoid performance impacts
- Log messages at `TRACE` and `DEBUG` levels are not evaluated in release builds unless those levels are explicitly enabled
- File logging uses a non-blocking writer to prevent I/O operations from blocking the UI thread
