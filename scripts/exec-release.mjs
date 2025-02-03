import {spawnSync} from 'child_process';
import {resolve} from 'path';

const releaseBinary = resolve(import.meta.dirname, `../target/release/zpm`);

process.exitCode = spawnSync(releaseBinary, process.argv.slice(2), {
  stdio: `inherit`,
}).status;
