#!/usr/bin/env node
// Generate BFish PNG/ICO icons without native image dependencies.
import { mkdirSync, writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const crcTable = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function inRoundedRect(x, y, size, radius) {
  const rx = Math.min(Math.max(x, radius), size - radius);
  const ry = Math.min(Math.max(y, radius), size - radius);
  return (x - rx) ** 2 + (y - ry) ** 2 <= radius ** 2;
}

function pixel(x, y, size) {
  if (!inRoundedRect(x, y, size, size * 0.22)) return [0, 0, 0, 0];

  const nx = x / size;
  const ny = y / size;

  // Body of the fish.
  const body = ((nx - 0.56) / 0.245) ** 2 + ((ny - 0.5) / 0.165) ** 2 <= 1;
  // Tail.
  const tail = nx > 0.18 && nx < 0.36 && Math.abs(ny - 0.5) < (nx - 0.18) * 1.05;
  // Top fin.
  const fin = nx > 0.44 && nx < 0.66 && ny < 0.38 && ny > 0.28 && (ny - 0.28) / 0.1 > Math.abs(nx - 0.55) / 0.12;
  // Eye.
  const eye = (nx - 0.66) ** 2 + (ny - 0.455) ** 2 < 0.028 ** 2;

  if (eye) return [15, 118, 110, 255];
  if (body || tail || fin) return [236, 253, 245, 255];
  return [15, 118, 110, 255];
}

function makePng(size) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    raw[y * (size * 4 + 1)] = 0;
    for (let x = 0; x < size; x += 1) {
      const [r, g, b, a] = pixel(x + 0.5, y + 0.5, size);
      const offset = y * (size * 4 + 1) + 1 + x * 4;
      raw[offset] = r;
      raw[offset + 1] = g;
      raw[offset + 2] = b;
      raw[offset + 3] = a;
    }
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function write(path, data) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, data);
  console.log(`wrote ${path} (${data.length} bytes)`);
}

write(join(root, 'assets', 'icon.png'), makePng(1024));

const iconDir = join(root, 'crates', 'bfish-app', 'icons');
write(join(iconDir, '32x32.png'), makePng(32));
write(join(iconDir, '128x128.png'), makePng(128));
write(join(iconDir, '128x128@2x.png'), makePng(256));
write(join(iconDir, 'icon.png'), makePng(512));

const icoPng = makePng(256);
const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(1, 4);
const entry = Buffer.alloc(16);
entry[0] = 0;
entry[1] = 0;
entry[2] = 0;
entry[3] = 0;
entry.writeUInt16LE(1, 4);
entry.writeUInt16LE(32, 6);
entry.writeUInt32LE(icoPng.length, 8);
entry.writeUInt32LE(22, 12);
write(join(iconDir, 'icon.ico'), Buffer.concat([header, entry, icoPng]));
