// Starts one product, fetches the given paths, prints status, content type and saves bodies.
//   node dump.ts <id> <path>...
import fs from 'node:fs';
import path from 'node:path';
import {Running, freePort, resetData, Q} from './products.ts';
const [id, ...paths] = process.argv.slice(2);
resetData(id);
const r = new Running(id, await freePort(), await freePort());
const s = await r.start();
console.log('start', JSON.stringify(s));
await new Promise(res => setTimeout(res, 500));
console.log('stdout:', r.out.slice(0, 600));
const out = path.join(Q, 'inspect', id);
fs.mkdirSync(out, {recursive: true});
for (const p of paths.length ? paths : ['/']) {
  const link = (r.out.match(/https?:\/\/\S+/) || [""])[0].replace(/^https?:\/\/[^\/]+/, "");
  const u = p === "LINK" ? link : p;
  try {
    const res = await fetch(r.base + u);
    const body = await res.text();
    console.log(u, res.status, res.headers.get('content-type'), body.length);
    fs.writeFileSync(path.join(out, (u.replace(/[^a-z0-9]+/gi, '_') || 'root') + '.txt'), body);
  } catch (e) { console.log(u, 'ERR', String(e)); }
}
await r.stop();
