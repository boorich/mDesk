use dioxus::prelude::*;
use crate::server_config::{ServerConfig, ServerConfigs};
use std::path::Path;
// use std::sync::Arc;
// use tokio::sync::Mutex;

/// Server list component
#[derive(Props, Clone, PartialEq)]
pub struct ServerManagerProps {
    pub on_select_server: EventHandler<ServerConfig>,
    #[props(default)]
    pub selected_id: Option<String>,
}

/// Component for managing MCP server configurations
#[component]
pub fn ServerManager(props: ServerManagerProps) -> Element {
    // State for server configurations
    let mut configs = use_signal(|| ServerConfigs::default());
    let mut show_add_dialog = use_signal(|| false);
    let mut is_editing = use_signal(|| false);
    let mut edit_server = use_signal(|| None::<ServerConfig>);
    let mut error_message = use_signal(|| None::<String>);
    
    // Load servers from the configuration file
    use_effect(move || {
        // Try to load from servers.json
        let config_path = Path::new("servers.json");
        
        match ServerConfigs::load_from_file(config_path) {
            Ok(loaded_configs) => {
                eprintln!("Loaded {} server configurations", loaded_configs.servers.len());
                configs.set(loaded_configs);
            }
            Err(e) => {
                eprintln!("Error loading server configurations: {}", e);
                error_message.set(Some(format!("Error loading configurations: {}", e)));
                
                // If the file doesn't exist or has errors, create a default configuration
                let default_configs = ServerConfigs::initialize_default();
                configs.set(default_configs.clone());
                
                // Try to save the default configuration
                if let Err(save_err) = default_configs.save_to_file(config_path) {
                    eprintln!("Error saving default configurations: {}", save_err);
                }
            }
        }
    });
    
    // Function to select a server
    let select_server = move |id: String| {
        if let Some(server) = configs.read().get_by_id(&id) {
            props.on_select_server.call(server.clone());
        }
    };
    
    // Function to add a new server configuration
    let mut add_server = move |_| {
        show_add_dialog.set(true);
        is_editing.set(false);
        edit_server.set(None);
    };
    
    // Function to edit an existing server configuration
    let mut edit_server_fn = move |id: String| {
        if let Some(server) = configs.read().get_by_id(&id) {
            edit_server.set(Some(server.clone()));
            is_editing.set(true);
            show_add_dialog.set(true);
        }
    };
    
    // Function to delete a server configuration
    let mut delete_server = move |id: String| {
        let mut configs_clone = configs.read().clone();
        
        if configs_clone.remove_server(&id) {
            // Ensure at least one server is marked as default
            configs_clone.ensure_default_exists();
            
            // Update state
            configs.set(configs_clone.clone());
            
            // Save the updated configuration
            let config_path = Path::new("servers.json");
            if let Err(e) = configs_clone.save_to_file(config_path) {
                error_message.set(Some(format!("Error saving configurations: {}", e)));
            }
        }
    };
    
    // Function to handle saving server from the dialog
    let submit_server = move |server: ServerConfig| {
        let mut configs_clone = configs.read().clone();
        
        if *is_editing.read() {
            configs_clone.update_server(server);
        } else {
            configs_clone.add_server(server);
        }
        
        // Update state
        configs.set(configs_clone.clone());
        
        // Close the dialog
        show_add_dialog.set(false);
        is_editing.set(false);
        edit_server.set(None);
        
        // Save the updated configuration
        let config_path = Path::new("servers.json");
        if let Err(e) = configs_clone.save_to_file(config_path) {
            error_message.set(Some(format!("Error saving configurations: {}", e)));
        }
    };
    
    // Function to close the dialog
    let close_dialog = move |_| {
        show_add_dialog.set(false);
        is_editing.set(false);
        edit_server.set(None);
    };
    
    rsx! {
        div { class: "server-manager",
            div { class: "server-manager-header",
                h2 { class: "server-manager-title", "MCP Servers" }
                
                button {
                    class: "add-server-button",
                    onclick: move |_| add_server(()),
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "16",
                        height: "16",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        line { x1: "12", y1: "5", x2: "12", y2: "19" }
                        line { x1: "5", y1: "12", x2: "19", y2: "12" }
                    }
                    "Add Server"
                }
            }
            
            if let Some(ref error) = *error_message.read() {
                div { class: "error-alert",
                    "{error}"
                }
            }
            
            if configs.read().servers.is_empty() {
                div { class: "empty-servers",
                    "No server configurations available. Click 'Add Server' to create one."
                }
            } else {
                div { class: "server-list",
                    {
                        // Create a vector of RSX nodes, one for each server
                        configs.read().servers.iter().map(|server| {
                            let server_id = server.id.clone();
                            let is_selected = props.selected_id.as_ref().map_or(false, |id| id == &server_id);
                            let server_name = server.name.clone();
                            let server_desc = server.description.clone();
                            let is_default = server.is_default;
                            
                            // Clone server_id for each closure
                            let select_id = server_id.clone();
                            let edit_id = server_id.clone();
                            let delete_id = server_id.clone();
                            
                            rsx! {
                                div {
                                    key: "{server_id}",
                                    class: if is_selected { "server-item selected" } else { "server-item" },
                                    div { class: "server-item-content",
                                        div { class: "server-icon",
                                            if is_default {
                                                div { class: "default-badge", "Default" }
                                            }
                                            svg {
                                                xmlns: "http://www.w3.org/2000/svg",
                                                width: "24",
                                                height: "24",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                stroke_linecap: "round",
                                                stroke_linejoin: "round",
                                                rect { 
                                                    x: "2", 
                                                    y: "2", 
                                                    width: "20", 
                                                    height: "8", 
                                                    rx: "2", 
                                                    ry: "2" 
                                                }
                                                rect { 
                                                    x: "2", 
                                                    y: "14", 
                                                    width: "20", 
                                                    height: "8", 
                                                    rx: "2", 
                                                    ry: "2" 
                                                }
                                                line { 
                                                    x1: "6", 
                                                    y1: "6", 
                                                    x2: "6.01", 
                                                    y2: "6" 
                                                }
                                                line { 
                                                    x1: "6", 
                                                    y1: "18", 
                                                    x2: "6.01", 
                                                    y2: "18" 
                                                }
                                            }
                                        }
                                        div { class: "server-info",
                                            div { class: "server-name", "{server_name}" }
                                            if let Some(ref desc) = server_desc {
                                                div { class: "server-description", "{desc}" }
                                            }
                                        }
                                    }
                                    div { class: "server-actions",
                                        button {
                                            class: "server-action select",
                                            onclick: move |_| select_server(select_id.clone()),
                                            "Select"
                                        }
                                        button {
                                            class: "server-action edit",
                                            onclick: move |_| edit_server_fn(edit_id.clone()),
                                            svg {
                                                xmlns: "http://www.w3.org/2000/svg",
                                                width: "16",
                                                height: "16",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                stroke_linecap: "round",
                                                stroke_linejoin: "round",
                                                path { d: "M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" }
                                                path { d: "M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" }
                                            }
                                        }
                                        button {
                                            class: "server-action delete",
                                            disabled: is_default,
                                            onclick: move |_| delete_server(delete_id.clone()),
                                            svg {
                                                xmlns: "http://www.w3.org/2000/svg",
                                                width: "16",
                                                height: "16",
                                                view_box: "0 0 24 24",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                                stroke_linecap: "round",
                                                stroke_linejoin: "round",
                                                path { d: "M3 6h18" }
                                                path { d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" }
                                            }
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
            
            // Server add/edit dialog
            if *show_add_dialog.read() {
                ServerDialog {
                    server: edit_server.read().clone(),
                    is_editing: *is_editing.read(),
                    on_submit: submit_server,
                    on_cancel: close_dialog,
                }
            }
        }
    }
}

// Server edit/add dialog component
#[derive(Props, Clone, PartialEq)]
pub struct ServerDialogProps {
    #[props(default)]
    pub server: Option<ServerConfig>,
    #[props(default)]
    pub is_editing: bool,
    pub on_submit: EventHandler<ServerConfig>,
    pub on_cancel: EventHandler<()>,
}

#[component]
fn ServerDialog(props: ServerDialogProps) -> Element {
    // Form state
    let mut id = use_signal(|| props.server.as_ref().map_or("".to_string(), |s| s.id.clone()));
    let mut name = use_signal(|| props.server.as_ref().map_or("".to_string(), |s| s.name.clone()));
    let mut command = use_signal(|| props.server.as_ref().map_or("docker".to_string(), |s| s.command.clone()));
    let mut args = use_signal(|| props.server.as_ref().map_or("".to_string(), |s| s.args.join(" ")));
    let mut description = use_signal(|| props.server.as_ref().map_or("".to_string(), |s| s.description.clone().unwrap_or_default()));
    let mut is_default = use_signal(|| props.server.as_ref().map_or(false, |s| s.is_default));
    
    let mut env_keys = use_signal(Vec::<String>::new);
    let mut env_values = use_signal(Vec::<String>::new);
    
    // Initialize environment variables
    use_effect(move || {
        if let Some(ref server) = props.server {
            let keys: Vec<String> = server.env.keys().cloned().collect();
            let values: Vec<String> = keys.iter().map(|k| server.env.get(k).unwrap_or(&String::new()).clone()).collect();
            
            env_keys.set(keys);
            env_values.set(values);
        }
    });
    
    // Add new environment variable
    let mut add_env_var = move |_| {
        env_keys.write().push("".to_string());
        env_values.write().push("".to_string());
    };
    
    // Remove environment variable
    let mut remove_env_var = move |index: usize| {
        if index < env_keys.read().len() {
            env_keys.write().remove(index);
            env_values.write().remove(index);
        }
    };
    
    // Submit form
    let submit = move |_| {
        // Create environment variables map
        let mut env = std::collections::HashMap::new();
        for (i, key) in env_keys.read().iter().enumerate() {
            if !key.is_empty() && i < env_values.read().len() {
                env.insert(key.clone(), env_values.read()[i].clone());
            }
        }
        
        // Parse arguments
        let args_vec = args.read()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        
        // Create server config
        let server = ServerConfig {
            id: id.read().clone(),
            name: name.read().clone(),
            command: command.read().clone(),
            args: args_vec,
            env,
            description: if description.read().is_empty() { None } else { Some(description.read().clone()) },
            is_default: *is_default.read(),
        };
        
        props.on_submit.call(server);
    };
    
    rsx! {
        div { class: "dialog-overlay",
            div { class: "server-dialog",
                div { class: "dialog-header",
                    h3 { class: "dialog-title", 
                        if props.is_editing { "Edit Server" } else { "Add Server" } 
                    }
                    button { 
                        class: "dialog-close",
                        onclick: move |_| props.on_cancel.call(()),
                        "×"
                    }
                }
                
                div { class: "dialog-content",
                    div { class: "form-group",
                        label { for: "server-id", "Server ID" }
                        input { 
                            id: "server-id",
                            class: "form-control",
                            value: "{id}",
                            disabled: props.is_editing,
                            placeholder: "e.g., filesystem",
                            oninput: move |e| id.set(e.value().clone())
                        }
                    }
                    
                    div { class: "form-group",
                        label { for: "server-name", "Display Name" }
                        input { 
                            id: "server-name",
                            class: "form-control",
                            value: "{name}",
                            placeholder: "e.g., Filesystem MCP",
                            oninput: move |e| name.set(e.value().clone())
                        }
                    }
                    
                    div { class: "form-group",
                        label { for: "server-command", "Command" }
                        input { 
                            id: "server-command",
                            class: "form-control",
                            value: "{command}",
                            placeholder: "e.g., docker",
                            oninput: move |e| command.set(e.value().clone())
                        }
                    }
                    
                    div { class: "form-group",
                        label { for: "server-args", "Arguments (space separated)" }
                        textarea { 
                            id: "server-args",
                            class: "form-control",
                            value: "{args}",
                            placeholder: "e.g., run -i --rm mcp/filesystem",
                            oninput: move |e| args.set(e.value().clone())
                        }
                    }
                    
                    div { class: "form-group",
                        label { for: "server-description", "Description (optional)" }
                        textarea { 
                            id: "server-description",
                            class: "form-control",
                            value: "{description}",
                            placeholder: "Brief description of this server",
                            oninput: move |e| description.set(e.value().clone())
                        }
                    }
                    
                    div { class: "form-check",
                        input { 
                            id: "server-default",
                            r#type: "checkbox",
                            class: "form-check-input",
                            checked: "{is_default}",
                            oninput: move |e| is_default.set(e.value().parse().unwrap_or(false))
                        }
                        label { for: "server-default", "Set as default server" }
                    }
                    
                    // Environment variables
                    div { class: "form-group",
                        div { class: "form-group-header",
                            label { "Environment Variables" }
                            button {
                                class: "btn-add-env",
                                onclick: move |_| add_env_var(()),
                                "+"
                            }
                        }
                        
                        div { class: "env-vars-list",
                            for (idx, key) in env_keys.read().clone().iter().enumerate() {
                                div {
                                    key: "{idx}",
                                    class: "env-var-item",
                                    input {
                                        class: "form-control env-key",
                                        placeholder: "Key",
                                        value: "{key}",
                                        oninput: move |e| {
                                            let mut keys = env_keys.read().clone();
                                            if idx < keys.len() {
                                                keys[idx] = e.value().clone();
                                                env_keys.set(keys);
                                            }
                                        }
                                    }
                                    input {
                                        class: "form-control env-value",
                                        placeholder: "Value",
                                        value: if idx < env_values.read().len() { env_values.read()[idx].clone() } else { "".to_string() },
                                        oninput: move |e| {
                                            let mut values = env_values.read().clone();
                                            if idx < values.len() {
                                                values[idx] = e.value().clone();
                                                env_values.set(values);
                                            }
                                        }
                                    }
                                    button {
                                        class: "btn-remove-env",
                                        onclick: move |_| remove_env_var(idx),
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }
                
                div { class: "dialog-footer",
                    button {
                        class: "btn-cancel",
                        onclick: move |_| props.on_cancel.call(()),
                        "Cancel"
                    }
                    button {
                        class: "btn-submit",
                        onclick: submit,
                        disabled: id.read().is_empty() || name.read().is_empty() || command.read().is_empty(),
                        if props.is_editing { "Update" } else { "Save" }
                    }
                }
            }
        }
    }
}
