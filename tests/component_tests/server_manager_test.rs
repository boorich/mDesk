#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    
    // Import components and types
    use m_desk_new::server_config::{ServerConfig, ServerConfigs};
    
    #[test]
    fn test_server_config_creation() {
        // Test that we can create a basic ServerConfig
        let config = ServerConfig {
            id: "test-server".to_string(),
            name: "Test Server".to_string(),
            command: "docker".to_string(),
            args: vec!["run".to_string(), "-i".to_string(), "--rm".to_string()],
            description: Some("Test server description".to_string()),
            is_default: false,
            env: HashMap::new(),
        };
        
        assert_eq!(config.id, "test-server");
        assert_eq!(config.name, "Test Server");
        assert_eq!(config.command, "docker");
        assert_eq!(config.args.len(), 3);
        assert_eq!(config.args[0], "run");
        assert_eq!(config.description, Some("Test server description".to_string()));
        assert!(!config.is_default);
    }
    
    #[test]
    fn test_server_configs_manipulation() {
        // Create ServerConfigs
        let mut configs = ServerConfigs::default();
        
        // Add a server (not marked as default)
        let config = ServerConfig {
            id: "test-server".to_string(),
            name: "Test Server".to_string(),
            command: "docker".to_string(),
            args: vec!["run".to_string()],
            description: None,
            is_default: false,  // explicitly not default
            env: HashMap::new(),
        };
        
        // Add another server to ensure we're not the only one
        let config2 = ServerConfig {
            id: "another-server".to_string(),
            name: "Another Server".to_string(),
            command: "docker".to_string(),
            args: vec!["run".to_string()],
            description: None,
            is_default: true,  // This one is the default
            env: HashMap::new(),
        };
        
        // Test add_server
        configs.add_server(config.clone());
        configs.add_server(config2.clone());
        
        // Test contains (via get_by_id)
        assert!(configs.get_by_id("test-server").is_some());
        
        // Test get_server
        let retrieved = configs.get_by_id("test-server");
        assert!(retrieved.is_some());
        let retrieved_config = retrieved.unwrap();
        assert_eq!(retrieved_config.id, "test-server");
        
        // Test remove_server (on non-default server)
        let removed = configs.remove_server("test-server");
        assert!(removed); // Should be successful
        assert!(configs.get_by_id("test-server").is_none());
        
        // Test that we can't remove a default server
        let removed = configs.remove_server("another-server");
        assert!(!removed); // Should fail
        assert!(configs.get_by_id("another-server").is_some()); // Should still exist
    }
} 