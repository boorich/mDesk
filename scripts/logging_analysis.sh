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

echo ""
echo "Suggested migration path:"
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
