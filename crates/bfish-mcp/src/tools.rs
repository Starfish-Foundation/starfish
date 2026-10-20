use bfish_core::ToolSpec;
use serde_json::{json, Value};

fn obj(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn integer(description: &str, minimum: u64, maximum: u64) -> Value {
    json!({
        "type": "integer",
        "description": description,
        "minimum": minimum,
        "maximum": maximum
    })
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    input_schema: Value,
    read_only: bool,
    destructive: bool,
) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        input_schema,
        read_only,
        destructive,
    }
}

/// The complete, vendor-neutral tool surface exposed by bfish-mcp.
///
/// Descriptions deliberately include path examples and return shapes because
/// smaller/local models do not infer those from names as reliably as frontier
/// models do.
pub fn all_tools() -> Vec<ToolSpec> {
    vec![
        tool(
            "studio_status",
            "Studio connection status",
            "Check whether Roblox Studio is connected to BFish and which place is open. \
             Call this first if any Studio tool fails, or before starting a multi-step task. \
             Returns { connected, port, sessions, recentLogs, calls }.",
            obj(json!({}), &[]),
            true,
            false,
        ),
        tool(
            "get_place_info",
            "Get place info",
            "Return the open place's name, PlaceId, GameId, and Workspace name. \
             Use this to confirm which experience you are editing before making changes.",
            obj(json!({}), &[]),
            true,
            false,
        ),
        tool(
            "get_file_tree",
            "Get DataModel tree",
            "Read the Studio explorer as a nested tree. Each node has name, className, path, \
             children, and (for scripts) hasSource/scriptType/enabled. \
             Start here to discover real instance paths; never guess a path. \
             Paths use dot notation, e.g. game.Workspace.Map.Spawn. \
             max_depth defaults to 4 and is capped at 10.",
            obj(
                json!({
                    "path": string("Root instance path. Defaults to game. Example: game.ServerScriptService"),
                    "max_depth": integer("How many levels of children to include. Default 4, max 10.", 1, 10)
                }),
                &[],
            ),
            true,
            false,
        ),
        tool(
            "get_services",
            "List services",
            "List the services present in the open place (Workspace, Players, ReplicatedStorage, \
             ServerScriptService, etc.) with their class names. Useful before creating scripts or \
             querying a specific service.",
            obj(json!({}), &[]),
            true,
            false,
        ),
        tool(
            "get_instance_children",
            "List instance children",
            "Return the direct or nested children of one instance. Each item includes name, \
             className, path, and childCount. Use this instead of downloading the whole tree when \
             you already know where to look.",
            obj(
                json!({
                    "path": string("Parent path. Example: game.ReplicatedStorage.Remotes"),
                    "max_depth": integer("Depth of descendants to include. Default 2, max 6.", 1, 6)
                }),
                &["path"],
            ),
            true,
            false,
        ),
        tool(
            "get_instance_properties",
            "Get instance properties",
            "Read properties from one instance. When `properties` is omitted a safe, useful set is \
             returned (Name, ClassName, Parent, Size, Position, Color, Transparency, Anchored, \
             CanCollide, Material, Shape, Text, Enabled, Value, Source length, etc.). \
             Typed values are returned as { _type: \"Vector3\", X, Y, Z } and similar objects.",
            obj(
                json!({
                    "path": string("Instance path. Example: game.Workspace.Door"),
                    "properties": json!({
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional list of property names, e.g. [\"Size\", \"Color\", \"Anchored\"]."
                    })
                }),
                &["path"],
            ),
            true,
            false,
        ),
        tool(
            "get_instance_attributes",
            "Get instance attributes",
            "Read all attributes on an instance as { name: { value, type } }. Attributes are often \
             used for gameplay tuning, so check them before changing behavior.",
            obj(
                json!({ "path": string("Instance path. Example: game.Workspace.Boss") }),
                &["path"],
            ),
            true,
            false,
        ),
        tool(
            "set_properties",
            "Set instance properties",
            "Set one or more properties on an instance inside a single undo recording. \
             Values may be plain JSON or typed objects: \
             {\"_type\":\"Vector3\",\"X\":0,\"Y\":10,\"Z\":0}, \
             {\"_type\":\"Color3\",\"R\":1,\"G\":0.2,\"B\":0.2}, \
             {\"_type\":\"UDim2\",\"X\":{\"Scale\":1,\"Offset\":0},\"Y\":{\"Scale\":1,\"Offset\":0}}, \
             {\"_type\":\"EnumItem\",\"value\":\"Enum.Material.Neon\"}. \
             Returns { changed, path }. Verify with get_instance_properties afterwards.",
            obj(
                json!({
                    "path": string("Instance path. Example: game.Workspace.Door"),
                    "properties": json!({
                        "type": "object",
                        "description": "Property name to value map. Example: {\"Anchored\": true, \"Color\": {\"_type\":\"Color3\",\"R\":1,\"G\":0.3,\"B\":0.1}}",
                        "additionalProperties": true
                    })
                }),
                &["path", "properties"],
            ),
            false,
            true,
        ),
        tool(
            "execute_luau",
            "Execute Luau in edit mode",
            "Run Luau in Studio's edit context. The code can read and modify the DataModel and uses \
             the same globals as a plugin script. `print` and `warn` output is captured; the return \
             value is serialized (tables become JSON). \
             Always return a small verification value or print a short summary. \
             Changes are wrapped in an undo recording. Avoid infinite loops. \
             Example: return game.Workspace:GetChildren(). \
             Returns { returnValue, output, durationMs }.",
            obj(
                json!({
                    "source": string("Luau source. Example: local p = Instance.new(\"Part\"); p.Name = \"Test\"; p.Parent = workspace; return p:GetFullName()"),
                    "timeout_seconds": integer("Client-side wait for completion. Default 120, max 300. This does not stop the Luau thread.", 5, 300)
                }),
                &["source"],
            ),
            false,
            true,
        ),
        tool(
            "search_instances",
            "Search instances and scripts",
            "Search the DataModel by name, class, or script source text. This is the safest way to \
             find an instance when you do not know its exact path. Returns matches with path, name, \
             className, and a short snippet for content matches.",
            obj(
                json!({
                    "query": string("Text to find (case-insensitive)."),
                    "search_type": json!({
                        "type": "string",
                        "enum": ["name", "class", "content"],
                        "description": "What to search. Default: name. Use content to grep script sources."
                    }),
                    "path": string("Optional search root. Defaults to game."),
                    "max_results": integer("Maximum matches. Default 50, max 200.", 1, 200)
                }),
                &["query"],
            ),
            true,
            false,
        ),
        tool(
            "get_script_source",
            "Read a script",
            "Read the full Source of a Script, LocalScript, or ModuleScript. Read before editing. \
             Returns { path, className, source, lineCount }.",
            obj(
                json!({ "path": string("Script path. Example: game.ServerScriptService.GameLoop") }),
                &["path"],
            ),
            true,
            false,
        ),
        tool(
            "set_script_source",
            "Write a script",
            "Replace a script's entire Source in one undo recording. Prefer this over execute_luau for \
             writing scripts: it is explicit and easy to undo. Read the existing source first unless the \
             user asked you to create a brand-new script. Verify by reading it back or checking syntax.",
            obj(
                json!({
                    "path": string("Existing script path. Example: game.ServerScriptService.GameLoop"),
                    "source": string("Complete new Luau source.")
                }),
                &["path", "source"],
            ),
            false,
            true,
        ),
        tool(
            "create_instance",
            "Create an instance",
            "Create an Instance and parent it in one undo recording. Supports any creatable class. \
             Set properties with typed values (see set_properties). \
             Example: { parent: \"game.Workspace\", class_name: \"Part\", name: \"Platform\", \
             properties: { \"Size\": { \"_type\": \"Vector3\", \"X\": 12, \"Y\": 1, \"Z\": 12 }, \
             \"Anchored\": true } }. Returns { path, className }.",
            obj(
                json!({
                    "parent": string("Parent path. Example: game.Workspace"),
                    "class_name": string("Roblox class name. Example: Part, Folder, Script, RemoteEvent, TextLabel"),
                    "name": string("Optional instance name."),
                    "properties": json!({
                        "type": "object",
                        "description": "Optional property map applied before parenting.",
                        "additionalProperties": true
                    })
                }),
                &["parent", "class_name"],
            ),
            false,
            true,
        ),
        tool(
            "clone_instance",
            "Clone an instance",
            "Clone an existing instance (with all descendants) and optionally rename or reparent the copy. \
             Useful for duplicating models, UI templates, or enemy rigs. Returns the new path.",
            obj(
                json!({
                    "path": string("Source instance path. Example: game.Workspace.Templates.Enemy"),
                    "parent": string("Optional destination parent. Defaults to the source's parent."),
                    "name": string("Optional name for the clone.")
                }),
                &["path"],
            ),
            false,
            true,
        ),
        tool(
            "delete_instance",
            "Delete an instance",
            "Destroy an instance and all descendants. This is destructive: confirm with the user first \
             when the instance contains meaningful work. The deletion is undoable in Studio. \
             Returns { deleted, path }.",
            obj(
                json!({ "path": string("Instance path to destroy.") }),
                &["path"],
            ),
            false,
            true,
        ),
        tool(
            "insert_asset",
            "Insert a Creator Store asset",
            "Insert a public Roblox asset by numeric asset id using InsertService. Scripts inside the \
             asset are removed for safety. Only use asset ids the user provided or that you found via \
             Roblox's public catalog. Returns { path, name, className }.",
            obj(
                json!({
                    "asset_id": json!({
                        "type": "integer",
                        "description": "Roblox asset id, e.g. 1033714."
                    }),
                    "parent": string("Optional parent path. Defaults to game.Workspace."),
                    "name": string("Optional name for the inserted model.")
                }),
                &["asset_id"],
            ),
            false,
            true,
        ),
        tool(
            "get_selection",
            "Get Studio selection",
            "Return the instances currently selected in Studio, with name, className, and path. \
             Useful when the user says \"this part\" or \"the selected object\".",
            obj(json!({}), &[]),
            true,
            false,
        ),
        tool(
            "set_selection",
            "Change Studio selection",
            "Change the Studio selection so the user can see what you are working on. \
             mode \"set\" replaces, \"add\" extends, \"remove\" subtracts; an empty array clears. \
             Returns { count, selection }.",
            obj(
                json!({
                    "paths": json!({
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Instance paths. Example: [\"game.Workspace.Door\"]"
                    }),
                    "mode": json!({
                        "type": "string",
                        "enum": ["set", "add", "remove"]
                    })
                }),
                &["paths"],
            ),
            true,
            false,
        ),
        tool(
            "get_console_output",
            "Read Studio output",
            "Return recent Output-window messages captured by the BFish plugin, including errors. \
             Use this after execute_luau, set_script_source, or a playtest to verify behavior. \
             Returns { logs: [{ at, level, message }], count }.",
            obj(
                json!({
                    "limit": integer("Newest N messages. Default 50, max 200.", 1, 200),
                    "level": json!({
                        "type": "string",
                        "enum": ["all", "info", "warning", "error"]
                    })
                }),
                &[],
            ),
            true,
            false,
        ),
        tool(
            "start_playtest",
            "Start a playtest",
            "Ask Studio to start a playtest. mode \"play\" starts a player simulation; \
             \"run\" starts server-only run mode. Returns immediately; use get_console_output to \
             observe startup errors. Studio must be in edit mode.",
            obj(
                json!({
                    "mode": json!({
                        "type": "string",
                        "enum": ["play", "run"],
                        "description": "Play (default) or run mode."
                    })
                }),
                &[],
            ),
            false,
            false,
        ),
        tool(
            "stop_playtest",
            "Stop a playtest",
            "Stop the active playtest and return Studio to edit mode. Returns { stopped }.",
            obj(json!({}), &[]),
            false,
            false,
        ),
        tool(
            "undo",
            "Undo in Studio",
            "Undo the most recent Studio change, including changes made by BFish. Returns { success }.",
            obj(json!({}), &[]),
            false,
            true,
        ),
        tool(
            "redo",
            "Redo in Studio",
            "Redo the most recently undone Studio change. Returns { success }.",
            obj(json!({}), &[]),
            false,
            true,
        ),
    ]
}

pub fn by_name(name: &str) -> Option<ToolSpec> {
    all_tools().into_iter().find(|t| t.name == name)
}
