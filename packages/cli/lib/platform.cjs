'use strict';

const targets = {
  'darwin-arm64': { os: 'darwin', cpu: 'arm64' },
  'darwin-x64': { os: 'darwin', cpu: 'x64' },
  'linux-arm64-gnu': { os: 'linux', cpu: 'arm64', libc: 'glibc' },
  'linux-x64-gnu': { os: 'linux', cpu: 'x64', libc: 'glibc' },
  'win32-x64': { os: 'win32', cpu: 'x64' },
};

function currentTarget(platform = process.platform, arch = process.arch, glibc) {
  if (platform === 'linux') {
    glibc ??= process.report?.getReport().header.glibcVersionRuntime;
    if (!glibc) return undefined;
    const [major, minor] = glibc.split('.').map(Number);
    if (!(major > 2 || (major === 2 && minor >= 35))) return undefined;
  }
  const key = `${platform}-${arch}${platform === 'linux' ? '-gnu' : ''}`;
  if (!Object.hasOwn(targets, key)) return undefined;
  const platformInfo = targets[key];
  return {
    key,
    name: `@request-xray/cli-${key}`,
    binary: `request-xray-native${platform === 'win32' ? '.exe' : ''}`,
    ...platformInfo,
  };
}

module.exports = { currentTarget, targets };
