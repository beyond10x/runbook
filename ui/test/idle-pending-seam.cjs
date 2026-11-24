// Preload of the server in idle-slow-page.spec.mjs (not a spec). Two seams, none in the code under test:
// - the keep-alive ping of the event streams (every 15 s) runs every 150 ms, or, with IDLE_PING=none, not within the hour;
// - once a `change` event was written to a stream, the response reports IDLE_PENDING bytes of unsent output (writableLength,
//   2 MiB when unset), as a page that does not read would hold, so that a case needs no megabytes of real output to stand at
//   the 1 MiB bound.
const http = require('node:http');
const realInterval = global.setInterval;
global.setInterval = (fn, ms, ...rest) => realInterval(fn, ms === 15000 ? (process.env.IDLE_PING === 'none' ? 3600000 : 150) : ms, ...rest);
const proto = http.ServerResponse.prototype;
let owner = proto;
let desc = null;
while (owner && !(desc = Object.getOwnPropertyDescriptor(owner, 'writableLength'))) owner = Object.getPrototypeOf(owner);
const realWrite = proto.write;
proto.write = function (chunk, ...rest) {
  if (typeof chunk === 'string' && chunk.startsWith('event: change')) this.idlePending = Number(process.env.IDLE_PENDING || 2 * 1048576);
  return realWrite.call(this, chunk, ...rest);
};
Object.defineProperty(proto, 'writableLength', {configurable: true, get() { return (desc && desc.get ? desc.get.call(this) : 0) + (this.idlePending || 0); }});
