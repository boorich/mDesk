# mDesk Development ToDo List

## Overview
This document outlines the next steps for developing mDesk, a native desktop application for managing MCP tools with OpenRouter LLM access. The goal is to create a productive, focused day of development with clear tasks and milestones.

## High Priority Tasks

### 1. Dioxus AI Development Tools Integration
- [ ] Set up and use [dioxus-ai](https://github.com/DioxusLabs/dioxus-ai) developer tools to improve our workflow
  - [ ] Component Generation Tool
    - [ ] Use the tool to generate complex UI components for the chat interface
    - [ ] Generate reusable components for resource and tool cards
  - [ ] Automated QA
    - [ ] Set up automated testing for critical application paths
    - [ ] Create test scenarios for MCP server operations

### 2. Chat Interface & OpenRouter Integration
- [x] Create a new tab in the UI for a Chat Interface
  - [x] Design and implement a modern chat window UI
  - [x] Add message history component with proper styling
  - [x] Create input area with send button and keyboard shortcuts
- [x] Implement OpenRouter API integration
  - [x] Create API client for OpenRouter
  - [x] Add model selection dropdown with available LLMs
  - [ ] Implement token tracking and usage monitoring
- [x] Develop tool selection algorithm
  - [x] Create logic to analyze user queries
  - [x] Implement tool selection based on query intent
  - [x] Add capability to chain multiple tools as needed

### 3. MCP Integration Improvements
- [x] Enhance error handling for MCP client connections
- [x] Implement proper loading states during MCP operations
- [x] Add persistent storage for MCP connection settings
- [x] Create a configuration panel for customizing MCP server settings

### 4. UI & UX Enhancements
- [ ] Add animations for state transitions
- [ ] Implement dark/light theme toggle
- [x] Create a collapsible sidebar
- [ ] Add tooltips for better user guidance
- [ ] Implement keyboard shortcuts for common actions

### 5. Tool Selection System Overhaul
- [x] Create RankedToolSelection system
- [x] Design and implement core selection structures (RankedToolSelection, ToolMatch)
- [x] Enhance LLM prompt engineering
  - [x] Context-aware tool selection
  - [x] Capture reasoning for selections
  - [x] Add validation feedback loop
- [x] Add configurable model selection with cost considerations
  - [x] Environment variable based model configuration
  - [x] Documentation of model options and tradeoffs
  - [x] Cost optimization guidance
- [x] Implement parameter validation against tool schemas
  - [x] Basic validation using jsonschema
  - [x] Parameter fixing for common issues
  - [x] LLM-based parameter correction
- [x] Add comprehensive logging and error reporting
  - [x] Structured logging with tracing
  - [x] Detailed validation statistics
  - [x] Error context and recovery tracking
- [x] Build validation and processing pipeline
  - [x] Input sanitization
    - [x] XSS prevention
    - [x] Length limits
    - [x] Depth checking
  - [x] Schema compliance checks
    - [x] Type validation
    - [x] Range validation
    - [x] Required fields
  - [x] Error recovery strategies
    - [x] Fallback values
    - [x] Default parameters
    - [x] Alternative tool suggestions
    - [x] Robust recovery from multiple issues
    - [x] Validation state tracking with recovery info
- [x] Implement caching and optimization
  - [x] Cache frequently used tool selections
    - [x] Time-based cache expiration
    - [x] Tool-based cache keys
    - [x] Smart caching decisions
    - [x] Cache statistics
    - [x] Cache invalidation
  - [x] Optimize LLM prompt size
  - [x] Batch processing for multiple tools
- [x] Add testing infrastructure
  - [x] Basic unit tests
  - [x] Integration tests with mock LLM
  - [ ] Performance benchmarks
  - [x] Validation test suite
    - [x] Tests for error recovery strategies
    - [x] Tests for tool selection caching
- [ ] Integration with chat system
  - [ ] Update chat interface to show alternative tools
  - [x] Tool suggestion UI implementation
  - [x] Add confidence threshold controls
  - [ ] Display error recovery messages with improved styling
  - [x] Cache statistics for debugging
- [ ] Prepare for agent mode
  - [ ] Design tool chaining interface
  - [ ] Implement decision tree logic for tool selection
  - [ ] Add self-correction strategies
- [x] Testing and validation
  - [x] Create comprehensive test suite for tool selection
  - [x] Add model configuration options for test cost optimization
  - [ ] Implement performance benchmarks
  - [x] Add validation pipeline tests

### 6. Advanced Tool Selection Integration
- [ ] Replace the regex-based tool detection with LLMToolSelector
  - [ ] Instantiate LLMToolSelector in the ChatTab component
  - [ ] Share the existing ToolSelectionCache with LLMToolSelector
  - [ ] Create method to call LLMToolSelector.select_tools() from the chat processing logic
  - [ ] Implement proper error handling for LLM-based tool selection
- [ ] Enhance the tool suggestion UI to use RankedToolSelection
  - [ ] Update the UI to show confidence scores from RankedToolSelection
  - [ ] Display reasoning for tool selections from the LLM
  - [ ] Show multiple tool suggestions when appropriate based on confidence thresholds
  - [ ] Implement selection between alternative tools
- [ ] Add testing for advanced tool selection
  - [ ] Create tests comparing regex-based and LLM-based tool selection
  - [ ] Test tool selection with various user queries
  - [ ] Benchmark performance and API cost of LLM-based selection
- [ ] Optimize LLM usage for tool selection
  - [ ] Implement selective LLM-based selection based on query complexity
  - [ ] Add heuristics to determine when to use simple vs. advanced selection
  - [ ] Create fallback mechanisms for when LLM API is unavailable

### 7. Tool Selection UX Improvements
- [ ] Add visualization for tool selection confidence
  - [ ] Create confidence indicator UI element
  - [ ] Show confidence scores in the tool suggestion UI
  - [ ] Implement confidence threshold adjustment in settings
- [ ] Enable user feedback on tool selections
  - [ ] Add thumbs up/down buttons for tool suggestions
  - [ ] Create feedback loop to improve future selections
  - [ ] Store successful selections to improve cache behavior
- [ ] Implement detailed tool parameter explanation
  - [ ] Show parameter descriptions from schema
  - [ ] Add example values for common parameters
  - [ ] Include validation feedback inline with parameters

## Medium Priority Tasks

### 1. Resource Management
- [ ] Implement resource creation/modification functionality
- [ ] Add filtering and searching capabilities for resources
- [ ] Create detailed resource view with additional metadata
- [ ] Implement resource export/import functionality

### 2. Tools Enhancement
- [ ] Create a detailed view for each tool
- [ ] Implement tool execution UI with parameter inputs
- [x] Add result visualization for tool outputs
- [ ] Create tool favorites or recently used section

### 3. Performance Optimization
- [ ] Profile the application for performance bottlenecks
- [ ] Implement lazy loading for resources and tools lists
- [ ] Optimize SVG renders and DOM updates
- [ ] Add request caching for frequently accessed data

## Automated Testing Tasks
- [ ] Check the [Everything MCP Server](https://github.com/modelcontextprotocol/servers/tree/HEAD/src/everything)

### 1. Unit Testing Setup
- [x] Create basic test structure for the project
- [x] Set up unit testing for core business logic
- [ ] Implement component testing for UI elements
  - [ ] Test Chat component rendering and message handling
  - [ ] Test ServerManager component functionality
  - [ ] Test ToolSuggestion and ToolExecution components
  - [ ] Test ToolManager and ToolTest components

### 2. Integration Testing
- [ ] Develop integration tests for the OpenRouter API client
- [x] Create tests for MCP server communication
- [x] Test tool selection and execution pipeline
- [ ] Test state management across components

### 3. End-to-End Testing
- [ ] Set up an E2E testing framework compatible with Dioxus
- [ ] Create test scenarios for common user journeys
  - [ ] Test the full chat conversation flow
  - [ ] Test resource creation and management
  - [ ] Test tool execution with different parameters
- [ ] Implement visual regression testing for UI components

### 4. Test Infrastructure
- [ ] Set up CI/CD pipeline for automated test execution
- [ ] Create test mocks for external dependencies (OpenRouter, MCP)
- [ ] Implement test coverage reporting
- [ ] Add performance benchmarking tests

## Next Steps (Prioritized)

### 1. Advanced Tool Selection Integration
- [ ] Replace the regex-based tool detection with LLMToolSelector
  - [ ] Instantiate LLMToolSelector in the ChatTab component
  - [ ] Share the existing ToolSelectionCache with LLMToolSelector
  - [ ] Create method to call LLMToolSelector.select_tools() from the chat processing logic
  - [ ] Implement proper error handling for LLM-based tool selection
- [ ] Enhance the tool suggestion UI to use RankedToolSelection
  - [ ] Update the UI to show confidence scores from RankedToolSelection
  - [ ] Display reasoning for tool selections from the LLM
  - [ ] Show multiple tool suggestions when appropriate based on confidence thresholds
  - [ ] Implement selection between alternative tools

### 2. Tool Selection UX Improvements
- [ ] Add visualization for tool selection confidence
  - [ ] Create confidence indicator UI element
  - [ ] Show confidence scores in the tool suggestion UI
- [ ] Enable user feedback on tool selections
  - [ ] Add thumbs up/down buttons for tool suggestions
- [ ] Implement detailed tool parameter explanation
  - [ ] Show parameter descriptions from schema
  - [ ] Add example values for common parameters

### 3. Chat System Improvements
- [ ] Update chat interface to show alternative tools
- [ ] Display error recovery messages with improved styling
- [ ] Implement token tracking and usage monitoring

### 4. Testing for Advanced Tool Selection
- [ ] Create tests comparing regex-based and LLM-based tool selection
- [ ] Test tool selection with various user queries
- [ ] Benchmark performance and API cost of LLM-based selection

## Future Enhancements

### Agent Mode Development
- [ ] Design tool chaining interface
- [ ] Implement decision tree logic for tool selection
- [ ] Add self-correction strategies

### Performance and Optimization
- [ ] Implement selective LLM-based selection based on query complexity
- [ ] Add heuristics to determine when to use simple vs. advanced selection
- [ ] Create fallback mechanisms for when LLM API is unavailable

### UI & UX Improvements
- [ ] Add animations for state transitions
- [ ] Implement dark/light theme toggle
- [ ] Add tooltips for better user guidance
- [ ] Implement keyboard shortcuts for common actions