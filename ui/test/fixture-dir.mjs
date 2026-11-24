// Where the specs write the run directories they build: SDC_UI_FIXTURE_DIR, else ui/target/fixture-runs. The hostile
// cases leave links to nothing and links to a directory that holds them; under a directory named target/ they stay out
// of a copy of the tree that leaves the build output behind, and scripts/check-tree-links.sh finds none elsewhere.
import path from 'node:path';
import {fileURLToPath} from 'node:url';

export const FIXTURE_DIR = process.env.SDC_UI_FIXTURE_DIR || path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'target', 'fixture-runs');
