// The programs the server runs and the slow lane's fixtures make stores with: one place, so a fixture
// built with one canon is acted on with the same one. SDC_UI_CANON and SDC_UI_DRIVE name them; without
// them the canon of a checkout beside this one and the driver built in this checkout are looked for.
// A path that is not there is refused by the caller with the path named (decisions.mjs, missingPrograms).
import path from 'node:path';

export const canonPath = (env = process.env) => env.SDC_UI_CANON || path.join(env.HOME || '/nonexistent', 'beyond10x/canon/target/debug/canon');
export const drivePath = (sdc, env = process.env) => env.SDC_UI_DRIVE || path.join(sdc, 'driver/target/debug/sdc-drive');
