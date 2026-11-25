#!/usr/bin/env node
'use strict';

const { spawn } = require('node:child_process');
const { createHash } = require('node:crypto');
const { accessSync, constants, lstatSync, readFileSync } = require('node:fs');
const { constants: { signals } } = require('node:os');
const path = require('node:path');
const { currentTarget } = require('../lib/platform.cjs');
const { version } = require('../package.json');

function fail(message, code = 4) {
  console.error(`request-xray: ${message}`);
  process.exitCode = code;
}

function nativeBinary() {
  const target = currentTarget();
  if (!target) {
    throw new Error(`Unsupported Node platform ${process.platform}/${process.arch}. Supported targets: macOS arm64/x64, Linux arm64/x64 glibc 2.35+, Windows x64.`);
  }
  let manifestPath;
  try {
    manifestPath = require.resolve(`${target.name}/package.json`);
  } catch {
    throw new Error(`Missing ${target.name}@${version}. Install the matching platform package with optional dependencies enabled, or install the matching local platform tarball.`);
  }
  try {
    if (lstatSync(manifestPath).size > 4096) throw new Error();
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    if (manifest.name !== target.name || manifest.version !== version ||
        manifest.xrayBinary?.file !== target.binary || !/^[a-f0-9]{64}$/.test(manifest.xrayBinary.sha256)) {
      throw new Error();
    }
    const binary = path.join(path.dirname(manifestPath), 'bin', target.binary);
    const stat = lstatSync(binary);
    if (!stat.isFile() || stat.size > 100 * 1024 * 1024) throw new Error();
    if (process.platform !== 'win32') accessSync(binary, constants.X_OK);
    if (createHash('sha256').update(readFileSync(binary)).digest('hex') !== manifest.xrayBinary.sha256) {
      throw new Error();
    }
    return binary;
  } catch {
    throw new Error(`Invalid or damaged native binary in ${target.name}@${version}. Reinstall the matching platform package.`);
  }
}

function main() {
  let binary;
  try { binary = nativeBinary(); } catch (error) {
    fail(error.message);
    return;
  }

  const child = spawn(binary, ['--managed-stdin', ...process.argv.slice(2)], {
    stdio: ['pipe', 'inherit', 'inherit'],
    windowsHide: true,
  });
  const shutdown = () => child.stdin.end();
  child.stdin.on('error', error => {
    if (error.code !== 'EPIPE' && error.code !== 'ERR_STREAM_DESTROYED') fail('Could not manage the native process input.', 1);
  });
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
  child.once('error', () => fail('Could not launch the native binary. Reinstall the matching platform package.'));
  child.once('close', (code, signal) => {
    process.off('SIGINT', shutdown);
    process.off('SIGTERM', shutdown);
    if (process.exitCode === undefined) process.exitCode = code ?? (128 + (signals[signal] ?? 1));
  });
}

main();
