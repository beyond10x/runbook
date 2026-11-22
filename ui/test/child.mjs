// The one way a test starts a server or another long-lived child: `spawnChild(command, args, options)`
// takes the options of child_process.spawn and returns the same ChildProcess. The child ends when the
// process that started it is gone, also when that process was ended with SIGKILL (guard.mjs). `proc.kill()`
// ends the child as before; `proc.childPid` holds the command's own pid once the guard has reported it
// (`await proc.started`). The guard has a process group of its own, so the group kill of an outer guard
// leaves an inner guard alive to end its own child. `{endWithParent: false}` starts the command as it is (the negative of the test
// in child.spec.mjs). Servers and players a person starts by hand never come through here: the product's
// behaviour does not change.
import {spawn} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const guard = path.join(path.dirname(fileURLToPath(import.meta.url)), 'guard.mjs');

export function spawnChild(command, args = [], {endWithParent = true, stdio = 'ignore', ...options} = {}) {
  if (!endWithParent) {
    const proc = spawn(command, args, {stdio, ...options});
    proc.childPid = proc.pid;
    proc.started = Promise.resolve(proc.pid);
    return proc;
  }
  const io = typeof stdio === 'string' ? [stdio, stdio, stdio] : [...stdio];
  const proc = spawn(process.execPath, [guard, String(process.pid), '--', command, ...args], {...options, detached: true, stdio: ['ignore', io[1] ?? 'ignore', io[2] ?? 'ignore', 'ipc']});
  proc.started = new Promise((resolve) => {
    proc.once('message', (m) => { proc.childPid = m.pid; proc.channel?.unref(); resolve(m.pid); });
    proc.once('exit', () => resolve(undefined));
  });
  return proc;
}
