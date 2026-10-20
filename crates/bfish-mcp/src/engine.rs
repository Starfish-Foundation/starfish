use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::bridge::Bridge;
use crate::tools;
use bfish_core::ToolSpec;

#[derive(Clone)]
pub struct Engine {
    bridge: Bridge,
}

impl Engine {
    pub fn new(port: u16) -> Self {
        Self {
            bridge: Bridge::new(port),
        }
    }

    pub fn bridge(&self) -> &Bridge {
        &self.bridge
    }

    pub fn tools(&self) -> Vec<ToolSpec> {
        tools::all_tools()
    }

    pub async fn call(&self, name: &str, args: &Value) -> Result<Value> {
        if name == "studio_status" {
            let status = self.bridge.status().await;
            let mut value = json!({
                "ok": true,
                "data": status
            });
            if !status.connected {
                value["hint"] = json!(
                    "Studio is not connected. Open Roblox Studio, then open the BFish Connector from the Plugins tab. If the plugin is not installed, use Settings → Studio in the BFish app."
                );
            }
            return Ok(value);
        }

        if tools::by_name(name).is_none() {
            bail!("Unknown tool `{name}`. Call tools/list to see the available BFish tools.");
        }

        let (endpoint, payload, timeout) = match name {
            "get_place_info" => ("/api/place-info", json!({}), 45),
            "get_file_tree" => (
                "/api/file-tree",
                json!({
                    "path": optional_string(args, "path").unwrap_or_else(|| "game".to_string()),
                    "maxDepth": args["max_depth"].as_u64().unwrap_or(4).clamp(1, 10)
                }),
                90,
            ),
            "get_services" => ("/api/services", json!({}), 45),
            "get_instance_children" => (
                "/api/children",
                json!({
                    "path": required_string(args, "path")?,
                    "maxDepth": args["max_depth"].as_u64().unwrap_or(2).clamp(1, 6)
                }),
                60,
            ),
            "get_instance_properties" => (
                "/api/properties",
                json!({
                    "path": required_string(args, "path")?,
                    "properties": args.get("properties").cloned()
                }),
                60,
            ),
            "get_instance_attributes" => (
                "/api/attributes",
                json!({ "path": required_string(args, "path")? }),
                45,
            ),
            "set_properties" => (
                "/api/set-properties",
                json!({
                    "path": required_string(args, "path")?,
                    "properties": required_object(args, "properties")?
                }),
                90,
            ),
            "execute_luau" => (
                "/api/execute-luau",
                json!({ "source": required_field(args, "source")? }),
                args["timeout_seconds"]
                    .as_u64()
                    .unwrap_or(120)
                    .clamp(5, 300),
            ),
            "search_instances" => (
                "/api/search",
                json!({
                    "query": required_string(args, "query")?,
                    "searchType": optional_string(args, "search_type").unwrap_or_else(|| "name".to_string()),
                    "path": optional_string(args, "path").unwrap_or_else(|| "game".to_string()),
                    "maxResults": args["max_results"].as_u64().unwrap_or(50).clamp(1, 200)
                }),
                120,
            ),
            "get_script_source" => (
                "/api/script-source",
                json!({ "path": required_string(args, "path")? }),
                45,
            ),
            "set_script_source" => (
                "/api/set-script-source",
                json!({
                    "path": required_string(args, "path")?,
                    "source": required_field(args, "source")?
                }),
                90,
            ),
            "create_instance" => (
                "/api/create-instance",
                json!({
                    "parent": required_string(args, "parent")?,
                    "className": required_string(args, "class_name")?,
                    "name": optional_string(args, "name"),
                    "properties": args.get("properties").cloned().unwrap_or_else(|| json!({}))
                }),
                60,
            ),
            "clone_instance" => (
                "/api/clone-instance",
                json!({
                    "path": required_string(args, "path")?,
                    "parent": optional_string(args, "parent"),
                    "name": optional_string(args, "name")
                }),
                60,
            ),
            "delete_instance" => (
                "/api/delete-instance",
                json!({ "path": required_string(args, "path")? }),
                60,
            ),
            "insert_asset" => {
                let asset_id = args["asset_id"].as_u64().ok_or_else(|| {
                    anyhow::anyhow!("`asset_id` must be a positive integer asset id.")
                })?;
                (
                    "/api/insert-asset",
                    json!({
                        "assetId": asset_id,
                        "parent": optional_string(args, "parent").unwrap_or_else(|| "game.Workspace".to_string()),
                        "name": optional_string(args, "name")
                    }),
                    120,
                )
            }
            "get_selection" => ("/api/selection", json!({}), 45),
            "set_selection" => {
                let paths = args["paths"].as_array().ok_or_else(|| {
                    anyhow::anyhow!("`paths` must be an array of instance paths.")
                })?;
                if paths.iter().any(|p| !p.is_string()) {
                    bail!("Every entry in `paths` must be a string instance path.");
                }
                (
                    "/api/set-selection",
                    json!({
                        "paths": paths,
                        "mode": optional_string(args, "mode").unwrap_or_else(|| "set".to_string())
                    }),
                    60,
                )
            }
            "get_console_output" => (
                "/api/logs",
                json!({
                    "limit": args["limit"].as_u64().unwrap_or(50).clamp(1, 200),
                    "level": optional_string(args, "level").unwrap_or_else(|| "all".to_string())
                }),
                45,
            ),
            "start_playtest" => (
                "/api/start-playtest",
                json!({
                    "mode": optional_string(args, "mode").unwrap_or_else(|| "play".to_string())
                }),
                60,
            ),
            "stop_playtest" => ("/api/stop-playtest", json!({}), 45),
            "undo" => ("/api/undo", json!({}), 45),
            "redo" => ("/api/redo", json!({}), 45),
            other => bail!("Tool `{other}` has no Studio binding."),
        };

        let result = self
            .bridge
            .call(endpoint, payload, Duration::from_secs(timeout))
            .await?;

        Ok(json!({ "ok": true, "data": result }))
    }
}

fn required_field(args: &Value, key: &str) -> Result<Value> {
    match args.get(key) {
        Some(value) if !value.is_null() => Ok(value.clone()),
        _ => bail!("Missing required argument `{key}`. Expected a JSON object with that field."),
    }
}

fn required_string(args: &Value, key: &str) -> Result<String> {
    let value = required_field(args, key)?;
    let value = value
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("`{key}` must be a string."))?
        .trim()
        .to_string();
    if value.is_empty() {
        bail!("`{key}` cannot be empty. Instance paths look like game.Workspace.Model.Part.");
    }
    Ok(value)
}

fn optional_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn required_object(args: &Value, key: &str) -> Result<Value> {
    let value = required_field(args, key)?;
    if !value.is_object() {
        bail!("`{key}` must be an object of property names to values.");
    }
    Ok(value)
}
