use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::engine::Engine;
use crate::tools;

const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "\
BFish connects you to a live Roblox Studio session. Start with studio_status, then use \
get_file_tree or search_instances to discover real instance paths before editing. Paths use \
dot notation such as game.Workspace.Model.Part. Use execute_luau for bulk work, \
set_script_source for writing scripts, and get_console_output to verify. Every Studio result \
is JSON containing ok:true on success. Never invent tool results.";

pub async fn http_handler(State(engine): State<Engine>, Json(payload): Json<Value>) -> Response {
    match engine.handle_json(payload).await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

pub async fn http_info(State(engine): State<Engine>) -> Json<Value> {
    Json(json!({
        "name": "bfish-mcp",
        "version": bfish_core::APP_VERSION,
        "transport": "streamable-http",
        "endpoint": "/mcp",
        "studioBridge": "/studio",
        "tools": engine.tools().len(),
        "instructions": INSTRUCTIONS
    }))
}

impl Engine {
    pub async fn handle_json(&self, payload: Value) -> Option<Value> {
        if let Some(batch) = payload.as_array() {
            let mut responses = Vec::new();
            for item in batch {
                if let Some(response) = self.handle_one(item.clone()).await {
                    responses.push(response);
                }
            }
            if responses.is_empty() {
                None
            } else {
                Some(Value::Array(responses))
            }
        } else {
            self.handle_one(payload).await
        }
    }

    async fn handle_one(&self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request["method"].as_str().unwrap_or_default().to_string();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

        // MCP notifications (initialized, cancelled, ...) never get a response.
        let id = id?;

        let result = match method.as_str() {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => {
                Ok(json!({ "tools": self.tools().iter().map(tool_json).collect::<Vec<_>>() }))
            }
            "tools/call" => Ok(self.call_tool(&params).await),
            "prompts/list" => Ok(json!({ "prompts": prompts_json() })),
            "prompts/get" => self.get_prompt(&params),
            "resources/list" => Ok(resources_json()),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "resources/read" => self.read_resource(&params).await,
            "logging/setLevel" => Ok(json!({})),
            other => Err(RpcError::method_not_found(other)),
        };

        Some(match result {
            Ok(value) => json!({ "jsonrpc": "2.0", "id": id, "result": value }),
            Err(err) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": err.code, "message": err.message }
            }),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params["protocolVersion"].as_str().unwrap_or("2025-06-18");
        let protocol = if SUPPORTED_PROTOCOLS.contains(&requested) {
            requested
        } else {
            "2025-06-18"
        };

        json!({
            "protocolVersion": protocol,
            "capabilities": {
                "tools": { "listChanged": false },
                "prompts": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false },
                "logging": {}
            },
            "serverInfo": {
                "name": "bfish-mcp",
                "title": "BFish",
                "version": bfish_core::APP_VERSION,
                "websiteUrl": "https://github.com/bfish-tools/bfish"
            },
            "instructions": INSTRUCTIONS
        })
    }

    async fn call_tool(&self, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or_default();
        if name.is_empty() {
            return tool_error("Missing tool name.");
        }
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        if tools::by_name(name).is_none() {
            return tool_error(format!(
                "Unknown tool `{name}`. Call tools/list to see the BFish tool surface."
            ));
        }

        match self.call(name, &arguments).await {
            Ok(value) => {
                let text =
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                let mut result = json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                });
                if value.is_object() {
                    result["structuredContent"] = value;
                }
                result
            }
            Err(err) => tool_error(format!("{err:#}")),
        }
    }

    fn get_prompt(&self, params: &Value) -> std::result::Result<Value, RpcError> {
        let name = params["name"].as_str().unwrap_or_default();
        let mut presets = bfish_core::prompts::presets();
        let index = presets
            .iter()
            .position(|p| p.id == name)
            .ok_or_else(|| RpcError {
                code: -32602,
                message: format!("Unknown prompt `{name}`."),
            })?;
        let preset = presets.remove(index);
        let task = params["arguments"]["task"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let text = match task {
            Some(task) => format!("{}\n\n# Current task\n{task}", preset.prompt),
            None => preset.prompt.to_string(),
        };

        Ok(json!({
            "description": preset.description,
            "messages": [{
                "role": "user",
                "content": { "type": "text", "text": text }
            }]
        }))
    }

    async fn read_resource(&self, params: &Value) -> std::result::Result<Value, RpcError> {
        let uri = params["uri"].as_str().unwrap_or_default();
        match uri {
            "bfish://status" => {
                let status = self.bridge().status().await;
                Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": serde_json::to_string_pretty(&status).unwrap_or_default()
                    }]
                }))
            }
            "bfish://tools" => Ok(json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "text/markdown",
                    "text": tools_markdown()
                }]
            })),
            _ => Err(RpcError {
                code: -32602,
                message: format!("Unknown resource `{uri}`."),
            }),
        }
    }
}

fn tool_json(tool: &bfish_core::ToolSpec) -> Value {
    json!({
        "name": tool.name,
        "title": tool.title,
        "description": tool.description,
        "inputSchema": tool.input_schema,
        "annotations": {
            "title": tool.title,
            "readOnlyHint": tool.read_only,
            "destructiveHint": tool.destructive,
            "idempotentHint": tool.read_only,
            "openWorldHint": false
        }
    })
}

fn tool_error(message: impl Into<String>) -> Value {
    json!({
        "content": [{ "type": "text", "text": format!("Error: {}", message.into()) }],
        "isError": true
    })
}

fn prompts_json() -> Vec<Value> {
    bfish_core::prompts::presets()
        .into_iter()
        .map(|preset| {
            json!({
                "name": preset.id,
                "title": preset.name,
                "description": preset.description,
                "arguments": [{
                    "name": "task",
                    "description": "Optional task to append to the system prompt.",
                    "required": false
                }]
            })
        })
        .collect()
}

fn resources_json() -> Value {
    json!({
        "resources": [
            {
                "uri": "bfish://status",
                "name": "Studio status",
                "description": "Live Roblox Studio connection status.",
                "mimeType": "application/json"
            },
            {
                "uri": "bfish://tools",
                "name": "BFish tool reference",
                "description": "Markdown reference for every BFish Studio tool.",
                "mimeType": "text/markdown"
            }
        ]
    })
}

fn tools_markdown() -> String {
    let mut out = String::from("# BFish Studio tools\n\n");
    for tool in tools::all_tools() {
        out.push_str(&format!("## `{}`\n\n{}\n\n", tool.name, tool.description));
        out.push_str("```json\n");
        out.push_str(&serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default());
        out.push_str("\n```\n\n");
    }
    out
}

struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
        }
    }
}

pub async fn serve_stdio(engine: Engine) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let payload = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(err) => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {err}") }
                });
                write_message(&mut stdout, &response).await?;
                continue;
            }
        };
        if let Some(response) = engine.handle_json(payload).await {
            write_message(&mut stdout, &response).await?;
        }
    }

    Ok(())
}

async fn write_message(stdout: &mut tokio::io::Stdout, value: &Value) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    stdout.write_all(&line).await?;
    stdout.flush().await?;
    Ok(())
}
