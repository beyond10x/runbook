// The confinement every product is built, tested and started in: the bubblewrap view of
// `sdc-free-agent`'s job confinement (driver/src/bin/sdc-free-agent.rs, base_view + job_bwrap):
// /usr and /etc read only, fresh /proc, /dev, /tmp, the Rust toolchain at /opt/rust, new PID, IPC
// and UTS namespaces, the whole root remounted read only, writable only the product copy and its
// cargo home, the home an empty tmpfs, empty environment plus PATH and the cargo variables.
//
//   node confine.ts build|test <product dir> <cargo home> [cargo args...]
//       cargo in the confinement, with --unshare-net; run it as `slot node confine.ts ...` so the
//       job draws on the slot pool (the jobserver fifo is bound in, as sdc-free-agent does).
//   node confine.ts serve <product dir> <cargo home> <binary> [args...]
//       a built binary in the same confinement without --unshare-net: it is started only from
//       the harness, which itself runs in a network namespace that holds nothing but loopback.
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const TOOLCHAIN = path.join(os.homedir(), '.rustup/toolchains/stable-x86_64-unknown-linux-gnu');

export function bwrapArgs(mode: 'cargo' | 'serve', dir: string, cargoHome: string, cmd: string[]): string[] {
  const home = path.join(dir, '.home');
  const a = ['--die-with-parent', '--new-session', '--unshare-pid', '--unshare-ipc', '--unshare-uts',
    '--ro-bind', '/usr', '/usr', '--symlink', 'usr/bin', '/bin', '--symlink', 'usr/bin', '/sbin',
    '--symlink', 'usr/lib', '/lib', '--symlink', 'usr/lib', '/lib64', '--ro-bind', '/etc', '/etc',
    '--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
    '--ro-bind', TOOLCHAIN, '/opt/rust', '--bind', cargoHome, cargoHome];
  if (mode === 'cargo') a.push('--unshare-net');
  a.push('--bind', dir, dir, '--tmpfs', home);
  const env: [string, string][] = [];
  for (const k of ['MAKEFLAGS', 'CARGO_MAKEFLAGS']) {
    const v = process.env[k];
    if (!v) continue;
    for (const part of v.split(/\s+/)) {
      const f = part.startsWith('--jobserver-auth=fifo:') ? part.slice('--jobserver-auth=fifo:'.length) : '';
      if (f && fs.existsSync(f) && !a.includes(f)) a.push('--bind', f, f);
    }
    env.push([k, v]);
  }
  a.push('--remount-ro', '/', '--chdir', dir, '--clearenv');
  env.push(['PATH', '/opt/rust/bin:/usr/bin'], ['HOME', home], ['LANG', 'C.UTF-8'], ['TERM', 'dumb'],
    ['TMPDIR', '/tmp'], ['CARGO_HOME', cargoHome], ['CARGO_NET_OFFLINE', 'true'], ['RUSTC', '/opt/rust/bin/rustc']);
  if (process.env.SLOT_JOB) env.push(['SLOT_JOB', process.env.SLOT_JOB]);
  for (const [k, v] of env) a.push('--setenv', k, v);
  a.push('--', ...cmd);
  return a;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [mode, dir0, ch0, ...rest] = process.argv.slice(2);
  const dir = path.resolve(dir0), cargoHome = path.resolve(ch0);
  fs.mkdirSync(cargoHome, {recursive: true});
  let args: string[];
  if (mode === 'build') args = bwrapArgs('cargo', dir, cargoHome, ['/opt/rust/bin/cargo', 'build', ...rest]);
  else if (mode === 'test') args = bwrapArgs('cargo', dir, cargoHome, ['/opt/rust/bin/cargo', 'test', ...rest]);
  else if (mode === 'serve') args = bwrapArgs('serve', dir, cargoHome, rest);
  else { console.error('mode: build|test|serve'); process.exit(2); }
  const c = spawn('/usr/bin/bwrap', args, {stdio: 'inherit', env: {}});
  c.on('exit', (code, sig) => process.exit(code ?? (sig ? 128 : 1)));
}
