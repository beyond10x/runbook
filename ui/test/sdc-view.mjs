// The SDC runbook's view (views/sdc.view.json) for the specs that call the page's modules or the
// server's functions directly, as the server does at start: `useSdcView()` at the top of a spec.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {setView} from '../web/view.js';

const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../views/sdc.view.json');
export const sdcView = JSON.parse(fs.readFileSync(file, 'utf8'));
export const useSdcView = () => setView(sdcView);
