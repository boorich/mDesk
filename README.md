# mDesk

A native desktop application for managing MCP (Model Context Protocol) tools with OpenRouter LLM access. mDesk enables you to connect to MCP servers, manage resources and tools, and interact with AI models through an intuitive interface.

![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)

## Features

- 🔌 Connect to multiple MCP servers simultaneously
- 🔍 Browse MCP resources and tools
- 💬 Chat with AI models via OpenRouter integration
- 🛠️ Execute MCP tools directly in chat
- ⚙️ Configure and manage server connections

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Dioxus CLI](https://dioxuslabs.com/learn/0.6/CLI/installation) for running the application
- Respective environments for running [MCP servers](https://github.com/modelcontextprotocol/servers)

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/boorich/mDesk.git
   cd mDesk
   ```

2. Create an environment file:
   ```bash
   # Create .env file with your OpenRouter API key
   echo "OPENROUTER_API_KEY=your_api_key_here" > .env
   ```

3. Run the application:
   ```bash
   # Run the application with Dioxus CLI
   dx serve --platform desktop
   ```

The application will automatically create a default server configuration on first run.

## Development

### Logging

mDesk uses a structured logging system based on the `tracing` crate. Logs are stored in `~/.mdesk/logs/` and are also displayed in the console during development.

You can configure log levels using the `RUST_LOG` environment variable in your `.env` file:

```
# Set global level to warn, but mDesk code to debug
RUST_LOG=warn,m_desk_new=debug
```

For more information, see the [logging documentation](docs/logging.md).

### Directory Structure

- `src/` - Application source code
  - `components/` - UI components
  - `openrouter/` - OpenRouter API integration
  - `logging/` - Logging configuration
- `assets/` - Static assets and stylesheets
- `public/` - Public assets served as-is
- `docs/` - Documentation

### Environment Variables

Create a `.env` file in the project root with:

```
OPENROUTER_API_KEY=your_openrouter_api_key

# Configure logging level
RUST_LOG=warn,m_desk_new=info
```

### Server Configuration

The application will automatically create a default `servers.json` file on first run. You can add, edit, or remove server configurations through the Server Settings tab in the application.

If you want to manually configure servers before running the application, you can create a `servers.json` file:

```json
{
  "servers": [
    {
      "id": "filesystem",
      "name": "Filesystem MCP",
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "--mount",
        "type=bind,src=/path/to/your/files,dst=/path/to/your/files",
        "mcp/filesystem",
        "/path/to/your/files"
      ],
      "env": {},
      "description": "Default filesystem MCP provider",
      "is_default": true
    }
  ]
}
```

### Commands

```bash
# Run development server (desktop platform)
dx serve --platform desktop

# Run development server (web platform)
dx serve --platform web
```

## Contributing

We welcome contributions! Here's how you can help:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Commit your changes (`git commit -m 'Add some amazing feature'`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
