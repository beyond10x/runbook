// The parent process of child.spec.mjs: `node test/child-parent.mjs <runs-dir> [plain]` starts the
// inspector's server through startServer (child.mjs; `plain` starts it without the guard), prints one
// JSON line {parentPid, server} with its own pid and the server's, and then waits until it is ended.
import {startServer} from './serve.mjs';

const [runs, mode] = process.argv.slice(2);
const s = await startServer(['--runs', runs], {endWithParent: mode !== 'plain'});
const server = await s.proc.started;
console.log(JSON.stringify({parentPid: process.pid, server}));
setInterval(() => {}, 1000);
