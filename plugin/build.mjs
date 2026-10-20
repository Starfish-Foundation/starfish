#!/usr/bin/env node
// Build the drag-and-drop Roblox Studio connector (.rbxmx) without Rojo.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const source = readFileSync(join(here, 'src', 'init.server.lua'), 'utf8');

const escape = (value) =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');

const xml = `<roblox xmlns:xmime="http://www.w3.org/2005/05/xmlmime" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="http://www.roblox.com/roblox.xsd" version="4">
  <Item class="Script" referent="RBX0">
    <Properties>
      <string name="Name">BFishConnector</string>
      <bool name="Disabled">false</bool>
      <ProtectedString name="Source">${escape(source)}</ProtectedString>
    </Properties>
  </Item>
</roblox>
`;

mkdirSync(join(here, 'dist'), { recursive: true });
const output = join(here, 'dist', 'BFishConnector.rbxmx');
writeFileSync(output, xml, 'utf8');
console.log(`Built ${output}`);
