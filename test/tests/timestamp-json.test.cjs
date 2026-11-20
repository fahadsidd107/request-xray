const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const path = require('node:path');
const { test } = require('node:test');

test('Rust timestamp JSON preserves adjacent nanoseconds above JS safe integer range', () => {
  const json = execFileSync(
    'cargo',
    ['run', '--locked', '--quiet', '-p', 'xray-core', '--example', 'timestamp_json'],
    { cwd: path.resolve(__dirname, '../..'), encoding: 'utf8' },
  );

  const values = JSON.parse(json);
  assert.deepEqual(values, ['0', '9007199254740992', '9007199254740993', '9223372036854775807']);
  const [epoch, earlier, later, maximum] = values.map(BigInt);
  assert.equal(later - earlier, 1n);
  assert.equal(maximum - epoch, (1n << 63n) - 1n);
});
