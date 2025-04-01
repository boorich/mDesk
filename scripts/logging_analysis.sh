#!/bin/bash
# This script analyzes the codebase for logging statements to help with migration
# from eprintln! to structured logging

echo "mDesk Logging Migration Analysis"
echo "================================"
echo ""

# Count eprintln! occurrences
EPRINTLN_COUNT=$(grep -r "eprintln!" --include="*.rs" src/ | wc -l)
echo "Found $EPRINTLN_COUNT eprintln! statements to migrate"

# Count existing tracing usage
TRACE_COUNT=$(grep -r "trace!" --include="*.rs" src/ | wc -l)
DEBUG_COUNT=$(grep -r "debug!" --include="*.rs" src/ | wc -l)
INFO_COUNT=$(grep -r "info!" --include="*.rs" src/ | wc -l)
WARN_COUNT=$(grep -r "warn!" --include="*.rs" src/ | wc -l)
ERROR_COUNT=$(grep -r "error!" --include="*.rs" src/ | wc -l)

echo ""
echo "Current tracing usage:"
echo "  trace!: $TRACE_COUNT"
echo "  debug!: $DEBUG_COUNT"
echo "  info!:  $INFO_COUNT"
echo "  warn!:  $WARN_COUNT"
echo "  error!: $ERROR_COUNT"

echo ""
echo "Files with most eprintln! statements:"
grep -r "eprintln!" --include="*.rs" src/ | cut -d: -f1 | sort | uniq -c | sort -nr | head -10

echo ""
echo "Common eprintln! patterns:"
grep -r "eprintln!" --include="*.rs" src/ | sed 's/.*eprintln!("\([^"]*\).*/\1/' | sort | uniq -c | sort -nr | head -10

# ======= Section for #[instrument] analysis =======

echo ""
echo "Instrumentation Analysis"
echo "========================"

# Count existing #[instrument] annotations
INSTRUMENT_COUNT=$(grep -r "#\[instrument" --include="*.rs" src/ | wc -l)
echo "Found $INSTRUMENT_COUNT existing #[instrument] annotations"

# Find async functions (good candidates for instrumentation)
echo ""
echo "Async functions without instrumentation (top candidates):"
grep -r "async fn" --include="*.rs" src/ | grep -v "#\[instrument" | cut -d: -f1,2 | head -15

# Find functions that return Result<> (error handling functions)
echo ""
echo "Error handling functions (Result-returning) without instrumentation:"
grep -r "fn.*Result" --include="*.rs" src/ | grep -v "#\[instrument" | grep -v "visit_" | grep -v "expecting" | head -15

# Find public functions (API boundaries)
echo ""
echo "Public functions without instrumentation (API boundaries):"
grep -r "pub fn" --include="*.rs" src/ | grep -v "#\[instrument" | grep -v "impl " | cut -d: -f1,2 | head -15

# Files already using instrumentation (for consistency)
echo ""
echo "Files already using #[instrument] (add to remaining functions for consistency):"
grep -r "#\[instrument" --include="*.rs" src/ | cut -d: -f1 | sort | uniq -c | sort -nr

# ======= New section for targeted recommendations =======

echo ""
echo "Targeted Implementation Plan"
echo "============================"

echo ""
echo "Phase 1: Core API and External Communication"
echo "--------------------------------------------"
echo "First, instrument these key external-facing functions:"

# OpenRouter client functions
echo "- OpenRouter API Client:"
grep -r "async fn" --include="*.rs" src/openrouter/ | grep -v "#\[instrument" | cut -d: -f1,2 | head -5

# Tool execution
echo "- Tool Execution:"
grep -r "async fn execute_tool" --include="*.rs" src/ | grep -v "#\[instrument" | cut -d: -f1,2

# Server management
echo "- Server Management:"
grep -r "fn.*server" --include="*.rs" src/components/server_manager.rs | grep -v "#\[instrument" | head -3

echo ""
echo "Phase 2: Business Logic and Tool Selection"
echo "-----------------------------------------"
echo "Next, instrument these business logic functions:"

# Tool selection
echo "- Tool Selection Logic:"
grep -r "fn select_tools" --include="*.rs" src/ | grep -v "#\[instrument" | cut -d: -f1,2
grep -r "async fn" --include="*.rs" src/components/tool_selection.rs | grep -v "#\[instrument" | cut -d: -f1,2 | head -3

# Parameter validation
echo "- Parameter Validation:"
grep -r "fn.*parameters" --include="*.rs" src/components/parameter_validation.rs | grep -v "#\[instrument" | cut -d: -f1,2 | head -3

echo ""
echo "Phase 3: Configuration and Utilities"
echo "-----------------------------------"
echo "Finally, instrument these supporting functions:"

# Configuration loading/saving
echo "- Configuration Management:"
grep -r "fn load_" --include="*.rs" src/ | grep -v "#\[instrument" | cut -d: -f1,2
grep -r "fn save_" --include="*.rs" src/ | grep -v "#\[instrument" | cut -d: -f1,2

# Initialization functions
echo "- Initialization:"
grep -r "fn init" --include="*.rs" src/ | grep -v "#\[instrument" | cut -d: -f1,2

# Suggested instrumentation pattern examples for different function types
echo ""
echo "Suggested Implementation Patterns"
echo "--------------------------------"
echo "For OpenRouter API client methods:"
echo '```rust'
echo '#[instrument(level = "debug", skip(self, messages), fields(model = model, num_messages = messages.len()))]'
echo 'pub async fn chat_completion(&self, model: &str, messages: Vec<ChatMessage>, ...) -> Result<ChatCompletionResponse, OpenRouterError> {'
echo '```'
echo ""
echo "For tool execution functions:"
echo '```rust'
echo '#[instrument(level = "debug", skip(arguments, mcp_state), fields(tool_name = %tool_name))]'
echo 'pub async fn execute_tool(tool_name: String, arguments: Value, mcp_state: &McpState) -> Result<CallToolResult, McpError> {'
echo '```'
echo ""
echo "For validation functions:"
echo '```rust'
echo '#[instrument(level = "debug", skip(parameters), fields(tool_name = %tool.name))]'
echo 'pub fn validate_parameters(tool: &Tool, parameters: &Value) -> Result<()> {'
echo '```'
echo ""
echo "For configuration functions:"
echo '```rust'
echo '#[instrument(level = "info", fields(config_path = %path.as_ref().display()))]'
echo 'pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {'
echo '```'

# Suggested priority list
echo ""
echo "Suggested instrumentation priority:"
echo "1. Async functions that perform I/O or external calls"
echo "2. Public API entry points and service methods"
echo "3. Error handling functions (returning Result)"
echo "4. Functions in files that already use #[instrument] (for consistency)"
echo "5. Functions with complex logic or multiple return paths"

# Recommendation for skip attributes
echo ""
echo "Tips for using #[instrument]:"
echo "- Use level=\"debug\" for most functions: #[instrument(level = \"debug\")]"
echo "- Skip large/sensitive parameters: #[instrument(skip(large_param, sensitive_data))]"
echo "- Add custom fields for context: #[instrument(fields(user_id = %user_id, request_size = payload.len()))]"
echo "- Skip RwLock/Mutex types to avoid serialization errors: #[instrument(skip(self))]"
echo "- Use % prefix for formatting Display traits: fields(tool_name = %tool.name)"

# Statistics on instrumentation
echo ""
echo "Instrumentation Statistics:"
TOTAL_FUNCTIONS=$(grep -r "fn " --include="*.rs" src/ | wc -l)
INSTRUMENTED_FUNCTIONS=$INSTRUMENT_COUNT
PERCENTAGE=$(awk "BEGIN {printf \"%.1f\", ($INSTRUMENTED_FUNCTIONS / $TOTAL_FUNCTIONS) * 100}")
echo "Current instrumentation coverage: $INSTRUMENTED_FUNCTIONS / $TOTAL_FUNCTIONS functions ($PERCENTAGE%)"

# ======= Original conclusion section =======

echo ""
echo "Suggested logging migration path:"
echo "1. Start with error handling (replace eprintln! with error!)"
echo "2. Continue with informational messages (replace eprintln! with info!)"
echo "3. Add debug and trace statements for detailed troubleshooting"
echo "4. Use the log! macro during transition for critical components"
echo ""
echo "Remember to use appropriate log levels:"
echo "  error! - For failures requiring immediate attention"
echo "  warn!  - For concerning but not critical issues"
echo "  info!  - For significant events during normal operation"
echo "  debug! - For detailed troubleshooting data during development"
echo "  trace! - For very detailed function-level tracing"
