const { spawn } = require('node:child_process');
const path = require('node:path');

const mode = process.argv.includes('--otel') ? 'otel'
  : process.argv.includes('--xray') ? 'xray' : process.env.XRAY_MODE ?? 'off';
const children = new Set();
let deadline;
let closing = false;

function shutdown(code = 0) {
  if (closing) return;
  closing = true;
  process.exitCode = code;
  for (const child of children) child.kill('SIGTERM');
  deadline = setTimeout(() => {
    for (const child of children) child.kill('SIGKILL');
  }, 6500).unref();
}

for (const [service, file] of [['lab-downstream', 'downstream.js'], ['lab-checkout', 'main.js']]) {
  const child = spawn(process.execPath, ['--require', path.resolve(__dirname, '../dist/instrumentation/register.js'), path.resolve(__dirname, '../dist', file)], {
    cwd: path.resolve(__dirname, '..'),
    env: { ...process.env, XRAY_MODE: mode, OTEL_SERVICE_NAME: service },
    stdio: 'inherit',
  });
  children.add(child);
  child.on('close', (code, signal) => {
    children.delete(child);
    if (!closing) {
      console.error(`${service} exited: ${code ?? signal}`);
      shutdown(code || 1);
    }
    if (!children.size) clearTimeout(deadline);
  });
  child.on('error', error => {
    console.error(`${service} failed to start: ${error.message}`);
    shutdown(1);
  });
}

process.on('SIGINT', () => shutdown());
process.on('SIGTERM', () => shutdown());
console.log(`Request X-Ray lab mode=${mode}; upstream=${process.env.LAB_PORT ?? 3900} downstream=${process.env.LAB_DOWNSTREAM_PORT ?? 3901}`);
