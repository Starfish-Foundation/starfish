--!nocheck
-- BFish Studio Connector
--
-- This plugin is intentionally small. It does not contain an AI chat UI.
-- It shows connection info and relays tool calls from the BFish desktop app /
-- bfish-mcp server into Studio. All model configuration lives in the app.
--
-- Security: the connector only talks to http://127.0.0.1 or http://localhost.

local HttpService = game:GetService("HttpService")
local ChangeHistoryService = game:GetService("ChangeHistoryService")
local Selection = game:GetService("Selection")
local LogService = game:GetService("LogService")
local RunService = game:GetService("RunService")
local ScriptContext = game:GetService("ScriptContext")

local PLUGIN_VERSION = "0.1.0"
local DEFAULT_URL = "http://127.0.0.1:34871"
local SETTING_URL = "BFishServerUrl"

local state = {
	url = DEFAULT_URL,
	sessionId = nil,
	connected = false,
	connecting = false,
	generation = 0,
	logs = {},
	lastError = nil,
}

local ui = nil

-- ---------------------------------------------------------------------------
-- Small utilities
-- ---------------------------------------------------------------------------

local function trim(value)
	return (tostring(value or ""):gsub("^%s+", ""):gsub("%s+$", ""))
end

local function normalizeUrl(raw)
	local url = trim(raw)
	if url == "" then
		return DEFAULT_URL
	end
	if not url:match("^https?://") then
		url = "http://" .. url
	end
	url = url:gsub("/mcp$", ""):gsub("/studio$", ""):gsub("/+$", "")
	return url
end

local function isLocalUrl(url)
	local host = url:match("^https?://([^/:]+)")
	host = host and host:lower() or ""
	return host == "127.0.0.1" or host == "localhost" or host == "[::1]"
end

local function truncate(value, maxLength)
	local text = tostring(value or "")
	if #text <= maxLength then
		return text
	end
	return text:sub(1, maxLength) .. "…"
end

local function pushLog(level, message)
	table.insert(state.logs, {
		at = os.date("!%Y-%m-%dT%H:%M:%SZ"),
		level = level,
		message = tostring(message),
	})
	while #state.logs > 300 do
		table.remove(state.logs, 1)
	end
	if ui and ui.logLabel then
		ui.logLabel.Text = "Last event: " .. truncate(message, 110)
	end
end

LogService.MessageOut:Connect(function(message, messageType)
	local level = "info"
	if messageType == Enum.MessageType.MessageWarning then
		level = "warning"
	elseif messageType == Enum.MessageType.MessageError then
		level = "error"
	end
	pushLog(level, message)
end)

ScriptContext.Error:Connect(function(message, stack)
	pushLog("error", tostring(message) .. "\n" .. tostring(stack))
end)

-- ---------------------------------------------------------------------------
-- Instance paths
-- ---------------------------------------------------------------------------

local function splitPath(path)
	local segments = {}
	local i = 1
	local n = #path
	while i <= n do
		local char = path:sub(i, i)
		if char == "." or char == "/" then
			i += 1
		elseif char == "[" then
			local quote = path:sub(i + 1, i + 1)
			if quote == '"' or quote == "'" then
				local j = i + 2
				local buffer = {}
				while j <= n do
					local current = path:sub(j, j)
					if current == "\\" then
						table.insert(buffer, path:sub(j + 1, j + 1))
						j += 2
					elseif current == quote then
						break
					else
						table.insert(buffer, current)
						j += 1
					end
				end
				table.insert(segments, table.concat(buffer))
				local close = path:find("]", j, true)
				i = close and (close + 1) or (n + 1)
			else
				local close = path:find("]", i, true) or n
				table.insert(segments, path:sub(i + 1, close - 1))
				i = close + 1
			end
		else
			local j = i
			while j <= n do
				local current = path:sub(j, j)
				if current == "." or current == "/" or current == "[" then
					break
				end
				j += 1
			end
			local segment = path:sub(i, j - 1)
			if segment ~= "" then
				table.insert(segments, segment)
			end
			i = j
		end
	end
	return segments
end

local function resolveInstance(path)
	if path == nil or path == "" or path == "game" then
		return game
	end
	local current = game
	for _, segment in ipairs(splitPath(tostring(path))) do
		if segment ~= "game" and segment ~= "" then
			local child = current:FindFirstChild(segment)
			if not child and current == game then
				local ok, service = pcall(function()
					return game:GetService(segment)
				end)
				if ok then
					child = service
				end
			end
			if not child then
				return nil,
					string.format(
						"Instance not found: %s (missing %q under %s)",
						tostring(path),
						segment,
						current:GetFullName()
					)
			end
			current = child
		end
	end
	return current
end

local function nameSegment(name)
	if name:match("^[%a_][%w_]*$") then
		return "." .. name
	end
	return '["' .. name:gsub("\\", "\\\\"):gsub('"', '\\"') .. '"]'
end

local function instancePath(instance)
	if instance == game then
		return "game"
	end
	local parts = {}
	local current = instance
	while current and current ~= game do
		table.insert(parts, 1, nameSegment(current.Name))
		current = current.Parent
	end
	return "game" .. table.concat(parts)
end

-- ---------------------------------------------------------------------------
-- Serialization
-- ---------------------------------------------------------------------------

local function serialize(value, depth)
	depth = depth or 0
	if depth > 6 then
		return tostring(value)
	end
	local kind = typeof(value)
	if kind == "Vector3" then
		return { _type = "Vector3", X = value.X, Y = value.Y, Z = value.Z }
	elseif kind == "Vector2" then
		return { _type = "Vector2", X = value.X, Y = value.Y }
	elseif kind == "Color3" then
		return { _type = "Color3", R = value.R, G = value.G, B = value.B }
	elseif kind == "CFrame" then
		local x, y, z = value:ToEulerAnglesXYZ()
		return {
			_type = "CFrame",
			Position = { X = value.X, Y = value.Y, Z = value.Z },
			Rotation = { X = x, Y = y, Z = z },
		}
	elseif kind == "UDim2" then
		return {
			_type = "UDim2",
			X = { Scale = value.X.Scale, Offset = value.X.Offset },
			Y = { Scale = value.Y.Scale, Offset = value.Y.Offset },
		}
	elseif kind == "UDim" then
		return { _type = "UDim", Scale = value.Scale, Offset = value.Offset }
	elseif kind == "BrickColor" then
		return { _type = "BrickColor", Name = value.Name }
	elseif kind == "EnumItem" then
		return { _type = "EnumItem", value = tostring(value) }
	elseif kind == "Instance" then
		return { _type = "Instance", path = instancePath(value), className = value.ClassName }
	elseif kind == "table" then
		local result = {}
		for key, item in pairs(value) do
			if type(key) == "string" or type(key) == "number" then
				result[key] = serialize(item, depth + 1)
			end
		end
		return result
	elseif kind == "number" then
		if value ~= value or value == math.huge or value == -math.huge then
			return tostring(value)
		end
		return value
	elseif kind == "string" or kind == "boolean" or kind == "nil" then
		return value
	end
	return tostring(value)
end

local function applyEnum(instance, property, rawValue)
	local text = tostring(rawValue or "")
	local enumType, itemName = text:match("^Enum%.([%w_]+)%.([%w_]+)$")
	if enumType then
		instance[property] = Enum[enumType][itemName]
		return
	end
	local current = instance[property]
	if typeof(current) == "EnumItem" then
		local item = current.EnumType[text]
		if item then
			instance[property] = item
			return
		end
	end
	error(string.format("Cannot apply enum value %q to property %q", text, property))
end

local function applyProperty(instance, property, raw)
	if type(raw) == "table" and raw._type then
		local kind = raw._type
		if kind == "Vector3" then
			instance[property] = Vector3.new(raw.X or 0, raw.Y or 0, raw.Z or 0)
		elseif kind == "Vector2" then
			instance[property] = Vector2.new(raw.X or 0, raw.Y or 0)
		elseif kind == "Color3" then
			instance[property] = Color3.new(raw.R or 0, raw.G or 0, raw.B or 0)
		elseif kind == "CFrame" then
			local position = raw.Position or {}
			local cframe = CFrame.new(position.X or 0, position.Y or 0, position.Z or 0)
			local rotation = raw.Rotation
			if rotation then
				cframe = cframe * CFrame.Angles(rotation.X or 0, rotation.Y or 0, rotation.Z or 0)
			end
			instance[property] = cframe
		elseif kind == "UDim2" then
			local x = raw.X or {}
			local y = raw.Y or {}
			instance[property] = UDim2.new(x.Scale or 0, x.Offset or 0, y.Scale or 0, y.Offset or 0)
		elseif kind == "UDim" then
			instance[property] = UDim.new(raw.Scale or 0, raw.Offset or 0)
		elseif kind == "BrickColor" then
			instance[property] = BrickColor.new(raw.Name or "Medium stone grey")
		elseif kind == "EnumItem" then
			applyEnum(instance, property, raw.value)
		else
			instance[property] = raw
		end
	else
		instance[property] = raw
	end
end

local function setProperties(instance, properties)
	local changed = {}
	for property, value in pairs(properties or {}) do
		if type(property) == "string" then
			applyProperty(instance, property, value)
			table.insert(changed, property)
		end
	end
	table.sort(changed)
	return changed
end

-- ---------------------------------------------------------------------------
-- Undo recordings
-- ---------------------------------------------------------------------------

local function beginRecording(label)
	local ok, recording = pcall(function()
		return ChangeHistoryService:TryBeginRecording(label)
	end)
	if ok then
		return recording
	end
	return nil
end

local function finishRecording(recording, label)
	if recording then
		pcall(function()
			ChangeHistoryService:FinishRecording(recording, Enum.FinishRecordingOperation.Commit)
		end)
	else
		pcall(function()
			ChangeHistoryService:SetWaypoint(label)
		end)
	end
end

local function withRecording(label, callback)
	local recording = beginRecording(label)
	local ok, result = pcall(callback)
	finishRecording(recording, label)
	if not ok then
		error(result, 0)
	end
	return result
end

-- ---------------------------------------------------------------------------
-- Studio handlers
-- ---------------------------------------------------------------------------

local setStatus

local handlers = {}

handlers["/api/ping"] = function(_data)
	return {
		pluginVersion = PLUGIN_VERSION,
		placeName = game.Name,
		placeId = game.PlaceId,
		studioVersion = tostring(version()),
		role = RunService:IsRunning() and "runtime" or "edit",
	}
end

handlers["/api/place-info"] = function(_data)
	return {
		placeName = game.Name,
		placeId = game.PlaceId,
		gameId = game.GameId,
		workspaceName = workspace.Name,
	}
end

handlers["/api/services"] = function(_data)
	local services = {}
	for _, child in ipairs(game:GetChildren()) do
		table.insert(services, {
			name = child.Name,
			className = child.ClassName,
			path = instancePath(child),
			childCount = #child:GetChildren(),
		})
	end
	table.sort(services, function(a, b)
		return a.name < b.name
	end)
	return { services = services, count = #services }
end

handlers["/api/file-tree"] = function(data)
	local maxDepth = math.clamp(tonumber(data.maxDepth) or 4, 1, 10)
	local root, err = resolveInstance(data.path or "game")
	if not root then
		error(err, 0)
	end

	local function build(instance, depth)
		local node = {
			name = instance.Name,
			className = instance.ClassName,
			path = instancePath(instance),
			children = {},
		}
		if instance:IsA("LuaSourceContainer") then
			node.hasSource = true
			node.scriptType = instance.ClassName
			if instance:IsA("BaseScript") then
				node.enabled = instance.Enabled
			end
		end
		local children = instance:GetChildren()
		if depth < maxDepth then
			for _, child in ipairs(children) do
				table.insert(node.children, build(child, depth + 1))
			end
		elseif #children > 0 then
			node.childCount = #children
			node.truncated = true
		end
		return node
	end

	return {
		tree = build(root, 0),
		rootPath = instancePath(root),
		maxDepth = maxDepth,
	}
end

handlers["/api/children"] = function(data)
	local maxDepth = math.clamp(tonumber(data.maxDepth) or 2, 1, 6)
	local root, err = resolveInstance(data.path)
	if not root then
		error(err, 0)
	end

	local function build(instance, depth)
		local entry = {
			name = instance.Name,
			className = instance.ClassName,
			path = instancePath(instance),
			childCount = #instance:GetChildren(),
		}
		if instance:IsA("LuaSourceContainer") then
			entry.hasSource = true
			entry.scriptType = instance.ClassName
		end
		if depth < maxDepth then
			entry.children = {}
			for _, child in ipairs(instance:GetChildren()) do
				table.insert(entry.children, build(child, depth + 1))
			end
		end
		return entry
	end

	local children = {}
	for _, child in ipairs(root:GetChildren()) do
		table.insert(children, build(child, 1))
	end
	return {
		path = instancePath(root),
		children = children,
		count = #children,
	}
end

local DEFAULT_PROPERTIES = {
	"Name",
	"ClassName",
	"Parent",
	"Archivable",
	"Size",
	"Position",
	"CFrame",
	"Color",
	"BrickColor",
	"Material",
	"Transparency",
	"Reflectance",
	"Anchored",
	"CanCollide",
	"CanQuery",
	"CanTouch",
	"CastShadow",
	"Shape",
	"Text",
	"TextSize",
	"TextColor3",
	"Visible",
	"BackgroundColor3",
	"BackgroundTransparency",
	"ZIndex",
	"Enabled",
	"Value",
	"Health",
	"MaxHealth",
	"WalkSpeed",
	"JumpPower",
	"Image",
	"SoundId",
	"Volume",
	"Looped",
	"Playing",
}

handlers["/api/properties"] = function(data)
	local instance, err = resolveInstance(data.path)
	if not instance then
		error(err, 0)
	end
	local requested = data.properties
	local names = DEFAULT_PROPERTIES
	if type(requested) == "table" and #requested > 0 then
		names = requested
	end
	local properties = {}
	for _, name in ipairs(names) do
		local ok, value = pcall(function()
			return instance[name]
		end)
		if ok and value ~= nil then
			properties[name] = serialize(value)
		end
	end
	return {
		path = instancePath(instance),
		className = instance.ClassName,
		properties = properties,
		childCount = #instance:GetChildren(),
	}
end

handlers["/api/attributes"] = function(data)
	local instance, err = resolveInstance(data.path)
	if not instance then
		error(err, 0)
	end
	local attributes = {}
	local count = 0
	for name, value in pairs(instance:GetAttributes()) do
		attributes[name] = { value = serialize(value), type = typeof(value) }
		count += 1
	end
	return { path = instancePath(instance), count = count, attributes = attributes }
end

handlers["/api/set-properties"] = function(data)
	local instance, err = resolveInstance(data.path)
	if not instance then
		error(err, 0)
	end
	if type(data.properties) ~= "table" then
		error("properties must be an object", 0)
	end
	return withRecording("BFish: set properties", function()
		local changed = setProperties(instance, data.properties)
		return { path = instancePath(instance), changed = changed, count = #changed }
	end)
end

handlers["/api/execute-luau"] = function(data)
	local source = data.source
	if type(source) ~= "string" or source == "" then
		error("source is required", 0)
	end
	local fn, compileError = loadstring(source)
	if not fn then
		error("Luau compile error: " .. tostring(compileError), 0)
	end

	return withRecording("BFish: execute Luau", function()
		local output = {}
		local function capture(level)
			return function(...)
				local parts = {}
				for index = 1, select("#", ...) do
					parts[index] = tostring(select(index, ...))
				end
				local line = table.concat(parts, " ")
				table.insert(output, line)
				while #output > 200 do
					table.remove(output, 1)
				end
				pushLog(level, "[execute_luau] " .. line)
			end
		end

		local oldPrint, oldWarn = print, warn
		print = capture("info")
		warn = capture("warning")
		local started = os.clock()
		local ok, result = xpcall(fn, function(message)
			return tostring(message) .. "\n" .. debug.traceback("", 2)
		end)
		print = oldPrint
		warn = oldWarn

		if not ok then
			error(result, 0)
		end

		return {
			returnValue = serialize(result),
			output = output,
			durationMs = math.floor((os.clock() - started) * 1000),
		}
	end)
end

handlers["/api/search"] = function(data)
	local query = tostring(data.query or "")
	if query == "" then
		error("query is required", 0)
	end
	local searchType = tostring(data.searchType or "name")
	local maxResults = math.clamp(tonumber(data.maxResults) or 50, 1, 200)
	local root, err = resolveInstance(data.path or "game")
	if not root then
		error(err, 0)
	end
	local needle = query:lower()
	local results = {}
	local scanned = 0
	local truncated = false

	local function walk(instance, depth)
		if #results >= maxResults or scanned >= 20000 then
			truncated = true
			return
		end
		scanned += 1
		local matched = false
		local snippet = nil
		if searchType == "class" then
			matched = instance.ClassName:lower():find(needle, 1, true) ~= nil
		elseif searchType == "content" then
			if instance:IsA("LuaSourceContainer") then
				local ok, source = pcall(function()
					return instance.Source
				end)
				if ok and type(source) == "string" then
					local start = source:lower():find(needle, 1, true)
					if start then
						matched = true
						snippet = source:sub(math.max(1, start - 80), math.min(#source, start + 160))
					end
				end
			end
		else
			matched = instance.Name:lower():find(needle, 1, true) ~= nil
		end
		if matched then
			table.insert(results, {
				name = instance.Name,
				className = instance.ClassName,
				path = instancePath(instance),
				hasSource = instance:IsA("LuaSourceContainer"),
				snippet = snippet,
			})
		end
		if depth < 12 then
			for _, child in ipairs(instance:GetChildren()) do
				if #results >= maxResults or scanned >= 20000 then
					truncated = true
					return
				end
				walk(child, depth + 1)
			end
		end
	end

	walk(root, 0)
	return {
		results = results,
		count = #results,
		query = query,
		searchType = searchType,
		scanned = scanned,
		truncated = truncated,
	}
end

handlers["/api/script-source"] = function(data)
	local instance, err = resolveInstance(data.path)
	if not instance then
		error(err, 0)
	end
	if not instance:IsA("LuaSourceContainer") then
		error(instance:GetFullName() .. " is not a script", 0)
	end
	local source = instance.Source
	local _, newlines = source:gsub("\n", "\n")
	return {
		path = instancePath(instance),
		className = instance.ClassName,
		source = source,
		lineCount = newlines + 1,
	}
end

handlers["/api/set-script-source"] = function(data)
	local instance, err = resolveInstance(data.path)
	if not instance then
		error(err, 0)
	end
	if not instance:IsA("LuaSourceContainer") then
		error(instance:GetFullName() .. " is not a script", 0)
	end
	if type(data.source) ~= "string" then
		error("source must be a string", 0)
	end
	return withRecording("BFish: write script", function()
		instance.Source = data.source
		return { path = instancePath(instance), bytes = #data.source }
	end)
end

handlers["/api/create-instance"] = function(data)
	local parent, err = resolveInstance(data.parent)
	if not parent then
		error(err, 0)
	end
	local className = tostring(data.className or "")
	if className == "" then
		error("className is required", 0)
	end
	return withRecording("BFish: create " .. className, function()
		local instance = Instance.new(className)
		if type(data.name) == "string" and data.name ~= "" then
			instance.Name = data.name
		end
		if type(data.properties) == "table" then
			setProperties(instance, data.properties)
		end
		instance.Parent = parent
		return {
			path = instancePath(instance),
			name = instance.Name,
			className = instance.ClassName,
		}
	end)
end

handlers["/api/clone-instance"] = function(data)
	local source, err = resolveInstance(data.path)
	if not source then
		error(err, 0)
	end
	local parent = source.Parent
	if data.parent then
		local resolved, parentError = resolveInstance(data.parent)
		if not resolved then
			error(parentError, 0)
		end
		parent = resolved
	end
	if not parent then
		parent = workspace
	end
	return withRecording("BFish: clone instance", function()
		local clone = source:Clone()
		if type(data.name) == "string" and data.name ~= "" then
			clone.Name = data.name
		end
		clone.Parent = parent
		return { path = instancePath(clone), name = clone.Name, className = clone.ClassName }
	end)
end

handlers["/api/delete-instance"] = function(data)
	local instance, err = resolveInstance(data.path)
	if not instance then
		error(err, 0)
	end
	if instance == game or instance == workspace then
		error("Refusing to delete " .. instance:GetFullName(), 0)
	end
	local path = instancePath(instance)
	local name = instance.Name
	return withRecording("BFish: delete instance", function()
		instance:Destroy()
		return { deleted = true, path = path, name = name }
	end)
end

handlers["/api/insert-asset"] = function(data)
	local assetId = tonumber(data.assetId)
	if not assetId or assetId <= 0 then
		error("assetId must be a positive number", 0)
	end
	local parent = workspace
	if data.parent then
		local resolved, err = resolveInstance(data.parent)
		if not resolved then
			error(err, 0)
		end
		parent = resolved
	end
	return withRecording("BFish: insert asset", function()
		local insertService = game:GetService("InsertService")
		local model = insertService:LoadAsset(assetId)
		for _, descendant in ipairs(model:GetDescendants()) do
			if descendant:IsA("LuaSourceContainer") then
				descendant:Destroy()
			end
		end
		if type(data.name) == "string" and data.name ~= "" then
			model.Name = data.name
		end
		model.Parent = parent
		return { path = instancePath(model), name = model.Name, className = model.ClassName }
	end)
end

local function selectionList()
	local items = Selection:Get()
	local result = {}
	for _, instance in ipairs(items) do
		table.insert(result, {
			name = instance.Name,
			className = instance.ClassName,
			path = instancePath(instance),
		})
	end
	return result
end

handlers["/api/selection"] = function(_data)
	local selection = selectionList()
	return { selection = selection, count = #selection }
end

handlers["/api/set-selection"] = function(data)
	local mode = tostring(data.mode or "set")
	local targets = {}
	for _, path in ipairs(data.paths or {}) do
		local instance = resolveInstance(path)
		if instance then
			table.insert(targets, instance)
		end
	end
	if mode == "add" then
		Selection:Add(targets)
	elseif mode == "remove" then
		Selection:Remove(targets)
	else
		Selection:Set(targets)
	end
	local selection = selectionList()
	return { count = #selection, selection = selection, mode = mode }
end

handlers["/api/logs"] = function(data)
	local limit = math.clamp(tonumber(data.limit) or 50, 1, 200)
	local level = tostring(data.level or "all")
	local filtered = {}
	for index = #state.logs, 1, -1 do
		local entry = state.logs[index]
		if level == "all" or entry.level == level then
			table.insert(filtered, 1, entry)
			if #filtered >= limit then
				break
			end
		end
	end
	return { logs = filtered, count = #filtered }
end

handlers["/api/start-playtest"] = function(data)
	local mode = tostring(data.mode or "play")
	if mode ~= "play" and mode ~= "run" then
		error('mode must be "play" or "run"', 0)
	end
	if RunService:IsRunning() then
		error("A playtest is already running", 0)
	end
	local ok, studioTestService = pcall(function()
		return game:GetService("StudioTestService")
	end)
	if not ok or not studioTestService then
		error("StudioTestService is not available in this Studio version", 0)
	end
	task.spawn(function()
		local started, testError = pcall(function()
			if mode == "run" then
				return studioTestService:ExecuteRunModeAsync({})
			end
			return studioTestService:ExecutePlayModeAsync({})
		end)
		if not started then
			pushLog("error", "Playtest failed: " .. tostring(testError))
			setStatus("Playtest failed", truncate(testError, 120), "error")
		end
	end)
	return { started = true, mode = mode }
end

handlers["/api/stop-playtest"] = function(_data)
	local ok, studioTestService = pcall(function()
		return game:GetService("StudioTestService")
	end)
	if not ok or not studioTestService then
		error("StudioTestService is not available in this Studio version", 0)
	end
	local stopped, stopError = pcall(function()
		studioTestService:EndTest()
	end)
	if not stopped then
		error("Could not stop playtest: " .. tostring(stopError), 0)
	end
	return { stopped = true }
end

handlers["/api/undo"] = function(_data)
	local ok, result = pcall(function()
		return ChangeHistoryService:Undo()
	end)
	if not ok then
		error("Undo failed: " .. tostring(result), 0)
	end
	return { success = result ~= false }
end

handlers["/api/redo"] = function(_data)
	local ok, result = pcall(function()
		return ChangeHistoryService:Redo()
	end)
	if not ok then
		error("Redo failed: " .. tostring(result), 0)
	end
	return { success = result ~= false }
end

-- ---------------------------------------------------------------------------
-- Local HTTP transport
-- ---------------------------------------------------------------------------

local function httpRequest(method, path, body)
	local options = {
		Url = state.url .. path,
		Method = method,
		Headers = {
			["Content-Type"] = "application/json",
			["Accept"] = "application/json",
		},
	}
	if body ~= nil then
		local encoded, payload = pcall(function()
			return HttpService:JSONEncode(body)
		end)
		if not encoded then
			return false, nil, "Could not encode command result: " .. tostring(payload), nil
		end
		options.Body = payload
	end
	local ok, response = pcall(function()
		return HttpService:RequestAsync(options)
	end)
	if not ok then
		return false, nil, tostring(response), nil
	end
	if not response.Success then
		local message = response.StatusMessage or "HTTP error"
		local responseBody = response.Body or ""
		if #responseBody > 0 then
			message = message .. " — " .. truncate(responseBody, 300)
		end
		return false, nil, message, response.StatusCode
	end
	if response.StatusCode == 204 or response.Body == "" then
		return true, {}
	end
	local decodedOk, decoded = pcall(function()
		return HttpService:JSONDecode(response.Body)
	end)
	if not decodedOk then
		return false, nil, "The BFish server returned invalid JSON", response.StatusCode
	end
	return true, decoded
end

local function friendlyError(message, statusCode)
	local text = tostring(message or "Unknown error")
	if text:find("Http requests are not enabled", 1, true) then
		return "Enable HTTP requests in Game Settings → Security, then reconnect."
	elseif statusCode == 404 then
		return "The BFish server restarted. Reconnecting…"
	elseif text:find("Connect", 1, true) or text:find("connection", 1, true) then
		return "Start the BFish app, then press Reconnect."
	end
	return truncate(text, 160)
end

local function executeCommand(command)
	local handler = handlers[command.endpoint]
	if not handler then
		return { ok = false, error = "Unknown endpoint: " .. tostring(command.endpoint) }
	end
	local ok, result = pcall(handler, command.data or {})
	if not ok then
		local message = tostring(result)
		pushLog("error", "[BFish] " .. truncate(message, 400))
		return { ok = false, error = message }
	end
	return { ok = true, result = serialize(result) }
end

setStatus = function(title, detail, kind)
	state.statusText = title
	state.detailText = detail
	if not ui then
		return
	end
	ui.statusLabel.Text = title
	ui.detailLabel.Text = detail
	local colors = {
		connected = Color3.fromRGB(45, 212, 191),
		connecting = Color3.fromRGB(250, 204, 21),
		error = Color3.fromRGB(248, 113, 113),
		disconnected = Color3.fromRGB(148, 163, 184),
	}
	ui.statusDot.BackgroundColor3 = colors[kind] or colors.disconnected
	ui.statusDot.UIStroke.Color = ui.statusDot.BackgroundColor3
end

local function refreshInfo()
	if not ui then
		return
	end
	ui.serverValue.Text = state.url
	ui.placeValue.Text = truncate(game.Name, 40)
	ui.placeIdValue.Text = tostring(game.PlaceId)
	ui.pluginValue.Text = PLUGIN_VERSION
	ui.studioValue.Text = tostring(version())
	ui.sessionValue.Text = state.sessionId and truncate(state.sessionId, 14) or "—"
end

local connect

local function pollLoop(generation)
	while generation == state.generation and state.sessionId do
		local ok, command, err, statusCode = httpRequest(
			"GET",
			"/studio/poll?session=" .. state.sessionId .. "&wait=25"
		)
		if generation ~= state.generation then
			return
		end
		if not ok then
			if statusCode == 404 then
				state.sessionId = nil
				state.connected = false
				state.connecting = true
				setStatus("Reconnecting…", "The BFish server restarted.", "connecting")
				refreshInfo()
				connect()
				return
			end
			state.connected = false
			state.connecting = false
			state.lastError = err
			setStatus("Not connected", friendlyError(err, statusCode), "disconnected")
			pushLog("warning", "Connection lost: " .. tostring(err))
			task.wait(2)
			if generation == state.generation then
				connect()
			end
			return
		end

		state.connected = true
		state.connecting = false
		setStatus("Connected", "Studio is ready for BFish.", "connected")
		if command and command.id then
			local outcome = executeCommand(command)
			local delivered = false
			for _ = 1, 3 do
				local sent = httpRequest("POST", "/studio/result", {
					sessionId = state.sessionId,
					id = command.id,
					ok = outcome.ok,
					result = outcome.result,
					error = outcome.error,
				})
				if sent then
					delivered = true
					break
				end
				task.wait(0.4)
			end
			if not delivered then
				pushLog("warning", "Could not deliver a tool result to BFish.")
			end
			setStatus("Connected", "Studio is ready for BFish.", "connected")
		end
	end
end

connect = function()
	state.generation += 1
	local generation = state.generation
	state.connecting = true
	state.connected = false

	if not isLocalUrl(state.url) then
		setStatus("Invalid server", "BFish only connects to localhost.", "error")
		return
	end

	setStatus("Connecting…", "Contacting " .. state.url, "connecting")
	refreshInfo()

	task.spawn(function()
		local ok, response, err, statusCode = httpRequest("POST", "/studio/hello", {
			pluginVersion = PLUGIN_VERSION,
			placeName = game.Name,
			placeId = game.PlaceId,
			studioVersion = tostring(version()),
			role = RunService:IsRunning() and "runtime" or "edit",
		})
		if generation ~= state.generation then
			return
		end
		if not ok or not response.sessionId then
			state.connecting = false
			state.lastError = err
			setStatus("Not connected", friendlyError(err, statusCode), "disconnected")
			pushLog("warning", "Could not reach BFish: " .. tostring(err))
			task.delay(2, function()
				if generation == state.generation then
					connect()
				end
			end)
			return
		end

		state.sessionId = response.sessionId
		state.connected = true
		state.connecting = false
		state.generation = state.generation
		setStatus("Connected", "Studio is ready for BFish.", "connected")
		refreshInfo()
		pushLog("info", "Connected to BFish at " .. state.url)
		pollLoop(generation)
	end)
end

-- ---------------------------------------------------------------------------
-- Connector UI
-- ---------------------------------------------------------------------------

local BACKGROUND = Color3.fromRGB(17, 19, 24)
local PANEL = Color3.fromRGB(27, 30, 37)
local PANEL_SOFT = Color3.fromRGB(34, 38, 46)
local BORDER = Color3.fromRGB(49, 55, 66)
local TEXT = Color3.fromRGB(236, 240, 245)
local MUTED = Color3.fromRGB(151, 161, 176)
local ACCENT = Color3.fromRGB(45, 212, 191)

local function create(class, properties, parent)
	local instance = Instance.new(class)
	for key, value in pairs(properties) do
		instance[key] = value
	end
	if parent then
		instance.Parent = parent
	end
	return instance
end

local function addCorner(parent, radius)
	create("UICorner", { CornerRadius = UDim.new(0, radius or 8) }, parent)
end

local function addRow(parent, labelText)
	local row = create("Frame", {
		BackgroundTransparency = 1,
		Size = UDim2.new(1, 0, 0, 22),
	}, parent)
	create("TextLabel", {
		BackgroundTransparency = 1,
		Size = UDim2.new(0.42, 0, 1, 0),
		Font = Enum.Font.Gotham,
		Text = labelText,
		TextColor3 = MUTED,
		TextSize = 12,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, row)
	local value = create("TextLabel", {
		BackgroundTransparency = 1,
		Position = UDim2.new(0.42, 0, 0, 0),
		Size = UDim2.new(0.58, 0, 1, 0),
		Font = Enum.Font.Gotham,
		Text = "—",
		TextColor3 = TEXT,
		TextSize = 12,
		TextTruncate = Enum.TextTruncate.AtEnd,
		TextXAlignment = Enum.TextXAlignment.Right,
	}, row)
	return value
end

local function buildUi()
	local toolbarOk, toolbar = pcall(function()
		return plugin:CreateToolbar("BFish")
	end)

	local info = DockWidgetPluginGuiInfo.new(Enum.InitialDockState.Right, false, false, 330, 540, 290, 380)
	local widget = plugin:CreateDockWidgetPluginGui("BFishConnector", info)
	widget.Title = "BFish Connector"
	widget.Name = "BFishConnector"

	if toolbarOk and toolbar then
		local buttonOk, button = pcall(function()
			return toolbar:CreateButton("Connector", "Open the BFish Studio Connector", "")
		end)
		if buttonOk and button then
			button.Click:Connect(function()
				widget.Enabled = not widget.Enabled
			end)
		end
	end

	local root = create("Frame", {
		BackgroundColor3 = BACKGROUND,
		BorderSizePixel = 0,
		Size = UDim2.fromScale(1, 1),
	}, widget)

	local header = create("Frame", {
		BackgroundTransparency = 1,
		Size = UDim2.new(1, 0, 0, 64),
	}, root)

	local logo = create("Frame", {
		BackgroundColor3 = ACCENT,
		Position = UDim2.new(0, 16, 0, 16),
		Size = UDim2.fromOffset(32, 32),
	}, header)
	addCorner(logo, 10)
	create("TextLabel", {
		BackgroundTransparency = 1,
		Size = UDim2.fromScale(1, 1),
		Font = Enum.Font.GothamBold,
		Text = "B",
		TextColor3 = Color3.fromRGB(10, 35, 32),
		TextSize = 18,
	}, logo)

	create("TextLabel", {
		BackgroundTransparency = 1,
		Position = UDim2.new(0, 58, 0, 14),
		Size = UDim2.new(1, -74, 0, 20),
		Font = Enum.Font.GothamBold,
		Text = "BFish",
		TextColor3 = TEXT,
		TextSize = 17,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, header)
	create("TextLabel", {
		BackgroundTransparency = 1,
		Position = UDim2.new(0, 58, 0, 34),
		Size = UDim2.new(1, -74, 0, 16),
		Font = Enum.Font.Gotham,
		Text = "Studio Connector",
		TextColor3 = MUTED,
		TextSize = 11,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, header)

	local content = create("ScrollingFrame", {
		BackgroundTransparency = 1,
		BorderSizePixel = 0,
		Position = UDim2.new(0, 12, 0, 64),
		Size = UDim2.new(1, -24, 1, -112),
		CanvasSize = UDim2.new(0, 0, 0, 0),
		AutomaticCanvasSize = Enum.AutomaticSize.Y,
		ScrollBarThickness = 4,
		ScrollBarImageColor3 = BORDER,
	}, root)
	create("UIListLayout", {
		Padding = UDim.new(0, 10),
		SortOrder = Enum.SortOrder.LayoutOrder,
	}, content)

	local statusCard = create("Frame", {
		BackgroundColor3 = PANEL,
		BorderSizePixel = 0,
		LayoutOrder = 1,
		Size = UDim2.new(1, 0, 0, 86),
	}, content)
	addCorner(statusCard, 10)
	create("UIStroke", { Color = BORDER, Thickness = 1 }, statusCard)

	local statusDot = create("Frame", {
		BackgroundColor3 = Color3.fromRGB(148, 163, 184),
		Position = UDim2.new(0, 14, 0, 18),
		Size = UDim2.fromOffset(10, 10),
	}, statusCard)
	addCorner(statusDot, 999)
	create("UIStroke", { Color = Color3.fromRGB(148, 163, 184), Thickness = 2 }, statusDot)

	local statusLabel = create("TextLabel", {
		BackgroundTransparency = 1,
		Position = UDim2.new(0, 34, 0, 12),
		Size = UDim2.new(1, -48, 0, 22),
		Font = Enum.Font.GothamBold,
		Text = "Starting…",
		TextColor3 = TEXT,
		TextSize = 14,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, statusCard)

	local detailLabel = create("TextLabel", {
		BackgroundTransparency = 1,
		Position = UDim2.new(0, 14, 0, 38),
		Size = UDim2.new(1, -28, 0, 34),
		Font = Enum.Font.Gotham,
		Text = "Preparing the connector.",
		TextColor3 = MUTED,
		TextSize = 12,
		TextWrapped = true,
		TextXAlignment = Enum.TextXAlignment.Left,
		TextYAlignment = Enum.TextYAlignment.Top,
	}, statusCard)

	local infoCard = create("Frame", {
		BackgroundColor3 = PANEL,
		BorderSizePixel = 0,
		LayoutOrder = 2,
		Size = UDim2.new(1, 0, 0, 172),
	}, content)
	addCorner(infoCard, 10)
	create("UIStroke", { Color = BORDER, Thickness = 1 }, infoCard)
	create("UIListLayout", {
		Padding = UDim.new(0, 2),
		SortOrder = Enum.SortOrder.LayoutOrder,
	}, infoCard)
	create("UIPadding", {
		PaddingLeft = UDim.new(0, 14),
		PaddingRight = UDim.new(0, 14),
		PaddingTop = UDim.new(0, 12),
		PaddingBottom = UDim.new(0, 10),
	}, infoCard)

	local serverValue = addRow(infoCard, "Server")
	local placeValue = addRow(infoCard, "Place")
	local placeIdValue = addRow(infoCard, "Place ID")
	local pluginValue = addRow(infoCard, "Plugin")
	local studioValue = addRow(infoCard, "Studio")
	local sessionValue = addRow(infoCard, "Session")

	local urlLabel = create("TextLabel", {
		BackgroundTransparency = 1,
		LayoutOrder = 3,
		Size = UDim2.new(1, 0, 0, 16),
		Font = Enum.Font.Gotham,
		Text = "SERVER URL",
		TextColor3 = MUTED,
		TextSize = 10,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, content)

	local urlBox = create("TextBox", {
		BackgroundColor3 = PANEL_SOFT,
		BorderSizePixel = 0,
		ClearTextOnFocus = false,
		Font = Enum.Font.Code,
		LayoutOrder = 4,
		PlaceholderText = DEFAULT_URL,
		Size = UDim2.new(1, 0, 0, 34),
		Text = state.url,
		TextColor3 = TEXT,
		TextSize = 12,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, content)
	addCorner(urlBox, 8)
	create("UIStroke", { Color = BORDER, Thickness = 1 }, urlBox)
	create("UIPadding", {
		PaddingLeft = UDim.new(0, 10),
		PaddingRight = UDim.new(0, 10),
	}, urlBox)

	local buttonRow = create("Frame", {
		BackgroundTransparency = 1,
		LayoutOrder = 5,
		Size = UDim2.new(1, 0, 0, 34),
	}, content)
	create("UIListLayout", {
		FillDirection = Enum.FillDirection.Horizontal,
		Padding = UDim.new(0, 8),
		SortOrder = Enum.SortOrder.LayoutOrder,
	}, buttonRow)

	local reconnectButton = create("TextButton", {
		AutoButtonColor = true,
		BackgroundColor3 = ACCENT,
		BorderSizePixel = 0,
		Font = Enum.Font.GothamBold,
		LayoutOrder = 1,
		Size = UDim2.new(0.5, -4, 1, 0),
		Text = "Reconnect",
		TextColor3 = Color3.fromRGB(8, 38, 34),
		TextSize = 12,
	}, buttonRow)
	addCorner(reconnectButton, 8)

	local copyButton = create("TextButton", {
		AutoButtonColor = true,
		BackgroundColor3 = PANEL_SOFT,
		BorderSizePixel = 0,
		Font = Enum.Font.Gotham,
		LayoutOrder = 2,
		Size = UDim2.new(0.5, -4, 1, 0),
		Text = "Copy info",
		TextColor3 = TEXT,
		TextSize = 12,
	}, buttonRow)
	addCorner(copyButton, 8)
	create("UIStroke", { Color = BORDER, Thickness = 1 }, copyButton)

	local hint = create("TextLabel", {
		BackgroundTransparency = 1,
		LayoutOrder = 6,
		Size = UDim2.new(1, 0, 0, 54),
		Font = Enum.Font.Gotham,
		Text = "Chat with BFish in the desktop app. This window only connects Studio; it does not store API keys.",
		TextColor3 = MUTED,
		TextSize = 11,
		TextWrapped = true,
		TextXAlignment = Enum.TextXAlignment.Left,
		TextYAlignment = Enum.TextYAlignment.Top,
	}, content)

	local logLabel = create("TextLabel", {
		BackgroundTransparency = 1,
		LayoutOrder = 7,
		Size = UDim2.new(1, 0, 0, 28),
		Font = Enum.Font.Code,
		Text = "Last event: —",
		TextColor3 = Color3.fromRGB(110, 120, 135),
		TextSize = 10,
		TextTruncate = Enum.TextTruncate.AtEnd,
		TextXAlignment = Enum.TextXAlignment.Left,
	}, content)

	local footer = create("TextLabel", {
		BackgroundTransparency = 1,
		Position = UDim2.new(0, 12, 1, -40),
		Size = UDim2.new(1, -24, 0, 28),
		Font = Enum.Font.Gotham,
		Text = "BFish Connector v" .. PLUGIN_VERSION .. "  ·  localhost only",
		TextColor3 = Color3.fromRGB(100, 110, 125),
		TextSize = 10,
		TextXAlignment = Enum.TextXAlignment.Center,
	}, root)

	ui = {
		widget = widget,
		root = root,
		statusDot = statusDot,
		statusLabel = statusLabel,
		detailLabel = detailLabel,
		serverValue = serverValue,
		placeValue = placeValue,
		placeIdValue = placeIdValue,
		pluginValue = pluginValue,
		studioValue = studioValue,
		sessionValue = sessionValue,
		logLabel = logLabel,
	}

	urlBox.FocusLost:Connect(function()
		local normalized = normalizeUrl(urlBox.Text)
		if not isLocalUrl(normalized) then
			setStatus("Invalid server", "BFish only connects to localhost.", "error")
			urlBox.Text = state.url
			return
		end
		state.url = normalized
		urlBox.Text = normalized
		plugin:SetSetting(SETTING_URL, normalized)
		connect()
	end)

	reconnectButton.MouseButton1Click:Connect(function()
		connect()
	end)

	copyButton.MouseButton1Click:Connect(function()
		local text = table.concat({
			"BFish Connector " .. PLUGIN_VERSION,
			"Server: " .. state.url,
			"Status: " .. tostring(state.statusText),
			"Place: " .. game.Name .. " (" .. tostring(game.PlaceId) .. ")",
			"Studio: " .. tostring(version()),
			"Session: " .. tostring(state.sessionId),
		}, "\n")
		local ok = pcall(function()
			setclipboard(text)
		end)
		copyButton.Text = ok and "Copied" or "Copy failed"
		task.delay(1.2, function()
			copyButton.Text = "Copy info"
		end)
		pushLog("info", "Copied connector diagnostics.")
	end)

	widget.Enabled = true
	refreshInfo()
end

-- ---------------------------------------------------------------------------
-- Start
-- ---------------------------------------------------------------------------

local savedUrl = plugin:GetSetting(SETTING_URL)
state.url = normalizeUrl(savedUrl or DEFAULT_URL)

buildUi()
setStatus("Connecting…", "Contacting " .. state.url, "connecting")
connect()

plugin.Unloading:Connect(function()
	state.generation += 1
	state.sessionId = nil
end)
