# MCP Testing Standard Proposal

## Overview
Adding standardized testing capabilities to the Model Context Protocol through a convention of server-exposed test tools.

## Motivation
- Clients need to verify their compatibility with MCP servers
- Server developers know best what functionality needs testing
- Current testing approaches are manual or non-standardized
- Need for automated verification of client implementations

## Specification

### Test Tool Convention
Tools with the prefix `mcp.test.*` are reserved for testing purposes and should:
1. Be recognized by clients as test tools
2. Be filterable from production tool lists
3. Follow standardized naming patterns

### Required Test Tools
Every MCP server should implement these basic test tools:
1. `mcp.test.capabilities` - Verify basic server capabilities
2. `mcp.test.protocol` - Test protocol-level functionality

### Optional Test Tools
Servers may implement additional test tools for:
1. Resource handling (`mcp.test.resources.*`)
2. Tool execution (`mcp.test.tools.*`)
3. Custom functionality (`mcp.test.custom.*`)

### Test Tool Schema
```typescript
interface TestTool {
  name: string;          // Must start with "mcp.test."
  description: string;   // Description of what is being tested
  inputSchema: {
    type: "object",
    properties: {
      // Test-specific parameters
    }
  }
  testMetadata: {
    category: string;    // e.g., "protocol", "resources", "tools"
    priority: number;    // 1 = critical, 2 = important, 3 = optional
    timeout: number;     // Maximum time in ms for test execution
  }
}
```

### Test Results Schema
```typescript
interface TestResult {
  success: boolean;
  message?: string;
  details?: {
    passed: string[];   // List of passed assertions
    failed: string[];   // List of failed assertions
    skipped: string[];  // List of skipped tests
  }
  duration: number;     // Time taken in ms
}
```

### Client Implementation Guidelines
1. Test Discovery
   - Query server for tools prefixed with `mcp.test.`
   - Parse test metadata and requirements
   - Filter test tools from production views

2. Test Execution
   - Execute tests in priority order
   - Respect test timeouts
   - Handle test failures gracefully
   - Report results in standardized format

3. Test Result Handling
   - Log test results for debugging
   - Report test coverage
   - Handle test failures appropriately

### Example Implementation

```typescript
// Server-side test tool implementation
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [{
      name: "mcp.test.protocol.basic",
      description: "Basic protocol compatibility test",
      inputSchema: {
        type: "object",
        properties: {
          echo: { type: "string" }
        },
        required: ["echo"]
      },
      testMetadata: {
        category: "protocol",
        priority: 1,
        timeout: 5000
      }
    }]
  };
});

// Client-side test execution
async function runServerTests(server: McpServer) {
  const tools = await server.listTools();
  const testTools = tools.filter(t => t.name.startsWith("mcp.test."));
  
  for (const tool of testTools.sort((a, b) => 
    a.testMetadata.priority - b.testMetadata.priority)) {
    const result = await executeTest(tool);
    // Handle result
  }
}
```

## Benefits
1. Standardized testing approach
2. Server-defined test requirements
3. Automated client verification
4. Self-documenting test specifications
5. Easy filtering in production

## Backwards Compatibility
- No changes to existing protocol
- Optional feature for servers
- Clients can ignore test tools
- Follows existing tool patterns

## Open Questions
1. Should we standardize test categories?
2. How to handle test dependencies?
3. Should we specify minimum required test coverage?
4. How to version test tools?

## Next Steps
1. Gather community feedback
2. Implement reference implementation
3. Update documentation
4. Create test tool examples 