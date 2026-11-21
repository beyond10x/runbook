// Runs a harness script in a new user + network namespace that has loopback and one dummy
// interface holding the default route. Nothing leaves the namespace (the dummy device drops every
// packet), but Chromium sees a network and so keeps navigator.onLine true and fires the
// 'offline'/'online' events when context.setOffline toggles. In a loopback-only namespace
// (bwrap --unshare-net) Chromium reports navigator.onLine false from the start and never fires
// those events, which breaks pages that wait for 'online' (probe-online.ts shows both cases).
//   slot node netns.ts <script.ts> <args...>
import {spawn} from 'node:child_process';
import path from 'node:path';

const here = path.dirname(new URL(import.meta.url).pathname);
const [script, ...args] = process.argv.slice(2);
const setup = 'ip link set lo up && ip link add d0 type dummy && ip link set d0 up && ' +
  'ip addr add 10.255.0.1/24 dev d0 && ip route add default dev d0';
const q = (s: string) => `'${s.replace(/'/g, `'\\''`)}'`;
const cmd = `${setup} && exec node --no-warnings ${[path.join(here, script), ...args].map(q).join(' ')}`;
const c = spawn('unshare', ['--user', '--map-root-user', '--net', 'sh', '-c', cmd], {stdio: 'inherit', cwd: here});
c.on('exit', (code, sig) => process.exit(code ?? (sig ? 128 : 1)));
