use serde_json::json;

use crate::engine::Engine;
use crate::tools;

#[test]
fn tool_names_are_unique_and_schemas_are_objects() {
    let all = tools::all_tools();
    assert!(all.len() >= 20);

    let mut names = std::collections::HashSet::new();
    for tool in &all {
        assert!(names.insert(tool.name.clone()), "duplicate {}", tool.name);
        assert_eq!(tool.input_schema["type"], "object", "{}", tool.name);
        assert!(
            tool.description.len() > 40,
            "{} needs a useful description",
            tool.name
        );
        assert!(!tool.title.is_empty());
    }
}

#[test]
fn core_tools_are_present() {
    for name in [
        "studio_status",
        "get_file_tree",
        "get_instance_properties",
        "set_properties",
        "execute_luau",
        "set_script_source",
        "create_instance",
        "search_instances",
        "start_playtest",
        "get_console_output",
    ] {
        assert!(tools::by_name(name).is_some(), "missing {name}");
    }
}

#[tokio::test]
async fn initialize_negotiates_protocol_and_lists_tools() {
    let engine = Engine::new(0);

    let response = engine
        .handle_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "clientInfo": { "name": "test", "version": "0" }
            }
        }))
        .await
        .expect("response");

    assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(response["result"]["serverInfo"]["name"], "bfish-mcp");
    assert_eq!(
        response["result"]["capabilities"]["tools"]["listChanged"],
        false
    );

    let list = engine
        .handle_json(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }))
        .await
        .expect("response");

    let tools = list["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), tools::all_tools().len());
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert!(names.contains(&"execute_luau"));
    assert!(names.contains(&"studio_status"));
}

#[tokio::test]
async fn notifications_get_no_response() {
    let engine = Engine::new(0);
    assert!(engine
        .handle_json(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await
        .is_none());
}

#[tokio::test]
async fn missing_arguments_are_actionable() {
    let engine = Engine::new(0);
    let error = engine
        .call("set_properties", &json!({}))
        .await
        .expect_err("expected error");
    assert!(format!("{error:#}").contains("path"));
}

#[tokio::test]
async fn studio_status_reports_disconnected() {
    let engine = Engine::new(0);
    let value = engine
        .call("studio_status", &json!({}))
        .await
        .expect("status");
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["connected"], false);
    assert!(value["hint"].as_str().is_some());
}

#[tokio::test]
async fn unknown_tools_are_rejected() {
    let engine = Engine::new(0);
    let response = engine
        .handle_json(json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": { "name": "does_not_exist", "arguments": {} }
        }))
        .await
        .expect("response");
    assert_eq!(response["result"]["isError"], true);
}

#[tokio::test]
async fn resources_and_prompts_are_available() {
    let engine = Engine::new(0);

    let prompts = engine
        .handle_json(json!({
            "jsonrpc":"2.0","id":3,"method":"prompts/list","params":{}
        }))
        .await
        .expect("response");
    assert!(prompts["result"]["prompts"].as_array().unwrap().len() >= 4);

    let resource = engine
        .handle_json(json!({
            "jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"bfish://tools"}
        }))
        .await
        .expect("response");
    let text = resource["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("execute_luau"));
}
