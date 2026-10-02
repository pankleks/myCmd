import { appendFile } from 'node:fs/promises';
import {
  checkVersion,
  setVersion,
  releaseNotes,
  collectAssets,
  checksums,
} from './release-lib.mjs';

const [command, ...args] = process.argv.slice(2);
try {
  switch (command) {
    case 'check': {
      if (args.length && (args[0] !== '--tag' || args.length !== 2))
        throw new Error('Usage: release.mjs check [--tag vX.Y.Z]');
      const { version, prerelease } = await checkVersion(
        process.cwd(),
        args[1],
      );
      await releaseNotes(process.cwd(), version);
      if (process.env.GITHUB_OUTPUT)
        await appendFile(
          process.env.GITHUB_OUTPUT,
          `version=${version}\nprerelease=${prerelease}\n`,
        );
      console.log(`Release metadata OK: ${version}`);
      break;
    }
    case 'version':
      if (args.length !== 1)
        throw new Error('Usage: npm run release:version -- X.Y.Z');
      await setVersion(process.cwd(), args[0]);
      console.log(
        `Version updated to ${args[0]}. Release notes in CHANGELOG.md are optional.`,
      );
      break;
    case 'notes': {
      const { version } = await checkVersion(process.cwd());
      process.stdout.write(await releaseNotes(process.cwd(), version));
      break;
    }
    case 'collect':
      if (args.length !== 3)
        throw new Error(
          'Usage: release.mjs collect BUNDLE_DIR OUTPUT_DIR PLATFORM',
        );
      console.log((await collectAssets(...args)).join('\n'));
      break;
    case 'checksums':
      if (args.length !== 1)
        throw new Error('Usage: release.mjs checksums ASSET_DIR');
      process.stdout.write(await checksums(args[0]));
      break;
    default:
      throw new Error('Commands: check, version, notes, collect, checksums');
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
