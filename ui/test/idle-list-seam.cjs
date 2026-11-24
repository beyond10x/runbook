// Preload of the server in idle-picker-window.spec.mjs (not a spec). Two seams, none in the code under test:
// - SIGHUP moves the clock of the process (Date.now) forward by 61 s, past the minute a served list counts for;
// - the longest list of states the server stored for a source (an array of {sig, at} given to Map.set) is written to the file
//   named by IDLE_SEAM_FILE on SIGUSR2, so that a case counts what is stored and does not wait to see its memory grow.
const fs = require('node:fs');
const realNow = Date.now;
let offset = 0;
Date.now = () => realNow() + offset;
process.on('SIGHUP', () => { offset += 61000; });
let longest = 0;
const realSet = Map.prototype.set;
Map.prototype.set = function (key, value) {
  if (Array.isArray(value) && value.length > longest && value.length && typeof value[0] === 'object' && value[0] !== null && typeof value[0].sig === 'string' && typeof value[0].at === 'number') longest = value.length;
  return realSet.call(this, key, value);
};
let answers = 0;
process.on('SIGUSR2', () => {
  const tmp = `${process.env.IDLE_SEAM_FILE}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify({longest, answers: ++answers}));
  fs.renameSync(tmp, process.env.IDLE_SEAM_FILE);
});
