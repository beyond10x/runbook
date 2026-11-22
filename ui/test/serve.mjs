// The inspector's server for a spec, on a port the kernel gives (--port 0, server.mjs prints the
// one it took): no fixed port, so a server of another session or of an earlier run cannot be the
// one a test talks to. `startServer(args, {env, cwd})` resolves with {url, port, proc} once the
// server has printed its address and answers, and rejects when it exits first or says nothing in 15 s.
// The server ends when the test process is gone (child.mjs); `endWithParent: false` starts it as it is.
import {spawnChild} from './child.mjs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ui = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function startServer(args, {env = process.env, cwd = ui, endWithParent = true} = {}) {
  return new Promise((resolve, reject) => {
    const proc = spawnChild(process.execPath, ['server.mjs', '--port', '0', ...args], {cwd, env, stdio: ['ignore', 'pipe', 'pipe'], endWithParent});
    let out = '', err = '', settled = false;
    const end = (fn, v) => { if (settled) return; settled = true; clearTimeout(timer); fn(v); };
    const timer = setTimeout(() => { proc.kill(); end(reject, new Error(`server.mjs printed no address in 15 s: ${out}${err}`)); }, 15000);
    proc.stderr.on('data', (d) => { if (!settled) err += d; });
    proc.stdout.on('data', (d) => {
      if (settled) return;
      out += d;
      const m = out.match(/sdc-ui on (http:\/\/127\.0\.0\.1:(\d+))\//);
      if (!m) return;
      const url = m[1], port = Number(m[2]);
      (async () => {
        for (let i = 0; i < 100; i++) {
          if (proc.exitCode != null) return end(reject, new Error(`server.mjs exited: ${err}`));
          try { await fetch(url + '/api/sources'); return end(resolve, {url, port, proc}); } catch { await new Promise((r) => setTimeout(r, 50)); }
        }
        proc.kill();
        end(reject, new Error(`server on ${url} does not answer`));
      })();
    });
    proc.on('exit', (code) => end(reject, new Error(`server.mjs exited with ${code} before it listened: ${err}${out}`)));
  });
}
