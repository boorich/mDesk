// This file acts as the test harness for integration tests

#[cfg(test)]
mod component_tests {
    // Include component test modules
    mod tool_suggestion_test;
    mod server_manager_test;
    mod tool_selection_test;
    mod parameter_validation_test;
    mod validation_pipeline_test;
}

#[cfg(test)]
mod utility_tests {
    // Utility function test modules
} 