# BFish Studio Connector

A deliberately small Roblox Studio plugin. It shows connection details and relays
tool calls from BFish into the open place. It does not contain an AI chat window
and never stores model API keys.

## Install

### One command

```bash
bfish-mcp --install-plugin
```

### From the app

Open **Settings → Studio → Install / update connector**.

### Manual

Build the model and copy it into your Roblox Plugins folder:

```bash
node build.mjs
# Windows
copy dist\BFishConnector.rbxmx "%LOCALAPPDATA%\Roblox\Plugins\"
# macOS
cp dist/BFishConnector.rbxmx ~/Documents/Roblox/Plugins/
```

Restart Studio, then open **BFish Connector** from the Plugins tab.

The connector connects to `http://127.0.0.1:34871` by default. Change the URL in
the connector if you run the bridge on another port. Only localhost URLs are
accepted.

## Rojo

```bash
rojo build default.project.json -o dist/BFishConnector.rbxmx
```
