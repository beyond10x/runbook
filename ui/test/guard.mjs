// The guard of a test child: `node test/guard.mjs <parent-pid> -- <command> [args...]` runs the command
// in a process group of its own and ends that whole group with SIGKILL when the parent's pid is gone,
// also when the parent was ended with SIGKILL and ran no teardown. Started through child.mjs
// (`spawnChild`), or by a config that starts a server (the command line above). It sends {pid} of the
// command over its IPC channel when it has one. Nothing here is looked up by name: the pid is the one given.
import {spawn} from 'node:child_process';

const [parentArg, sep, cmd, ...args] = process.argv.slice(2);
const parent = Number(parentArg);
if (!Number.isInteger(parent) || parent < 1 || sep !== '--' || !cmd) {
  console.error('usage: node guard.mjs <parent-pid> -- <command> [args...]');
  process.exit(2);
}

const alive = (pid) => { try { process.kill(pid, 0); return true; } catch (e) { return e.code === 'EPERM'; } };
const killGroup = (pid, signal) => { try { process.kill(-pid, signal); } catch { /* the group is gone */ } };

if (!alive(parent)) process.exit(1);

const child = spawn(cmd, args, {stdio: ['ignore', 'inherit', 'inherit'], detached: true});
child.on('error', (e) => { console.error(`guard: cannot start ${cmd}: ${e.message}`); process.exit(127); });
if (child.pid && process.send) process.send({pid: child.pid});

const end = (code) => { killGroup(child.pid, 'SIGKILL'); process.exit(code); };
const watch = setInterval(() => { if (!alive(parent)) end(1); }, 100);
child.on('exit', (code, signal) => { clearInterval(watch); end(code ?? 128 + (signal === 'SIGKILL' ? 9 : 15)); });

for (const signal of ['SIGTERM', 'SIGINT', 'SIGHUP']) {
  process.on(signal, () => {
    killGroup(child.pid, signal);
    setTimeout(() => end(1), 5000).unref();
  });
}
