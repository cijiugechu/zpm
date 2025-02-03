import {spawnSync} from 'child_process';
import {resolve} from 'path';

const releaseBinary = resolve(import.meta.dirname, `../artifacts/zpm-${process.platform}/zpm`);

process.exitCode = spawnSync(releaseBinary, process.argv.slice(2), {
  stdio: `inherit`,
}).status;
