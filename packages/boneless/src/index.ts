#!/usr/bin/env node
//
// Boneless: the janky Grebuloff build script
//

import child_process, {
  ChildProcess,
  SpawnOptionsWithoutStdio,
} from 'child_process';
import fs from 'fs';
import path from 'path';
import * as constants from './constants.js';

//
// Utility functions
//
async function execGet(
  cmd: string,
  extraOpts: SpawnOptionsWithoutStdio = {},
): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = child_process.spawn(
      cmd,
      Object.assign({ shell: true }, extraOpts),
    );

    let output = '';

    function recv(data: Buffer) {
      const str = data.toString();
      output += str;
    }

    child.stdout.on('data', recv);
    child.stderr.on('data', recv);

    child.on('close', (code) => {
      if (code === 0) {
        resolve(output);
      } else {
        reject(`Process exited with code ${code}`);
      }
    });
  });
}

async function exec(cmd: string, extraOpts: SpawnOptionsWithoutStdio = {}) {
  return new Promise<void>((resolve, reject) => {
    const child: ChildProcess = child_process.spawn(
      cmd,
      Object.assign({ shell: true, stdio: 'inherit' }, extraOpts),
    );

    child.on('close', (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(`Process exited with code ${code}`);
      }
    });
  });
}

async function withProjects(
  func: (
    name: string,
    meta: constants.ProjectMeta,
  ) => unknown | Promise<unknown>,
  projects: string | string[] = Object.keys(constants.PROJECTS),
) {
  if (typeof projects === 'string') {
    projects = [projects];
  }

  const ret: unknown[] = [];
  for (const p of projects) {
    const meta = constants.PROJECTS[p];
    const pret = func(p, meta);
    if (pret !== undefined) {
      if (pret instanceof Promise) {
        await pret;
      }

      if (pret !== undefined) {
        ret.push(pret);
      }
    }
  }

  return ret;
}

async function execFor(
  project: string | string[],
  cmd: string,
  extraOpts = {},
) {
  await withProjects(async (name, meta) => {
    await exec(cmd, Object.assign({ cwd: meta.dir }, extraOpts));
  }, project);
}

async function copyArtifact(file: string) {
  if (!fs.existsSync(constants.OUTPUT_DIR)) {
    fs.mkdirSync(constants.OUTPUT_DIR);
  }

  const src = path.join(constants.WORKSPACE, file);
  const dest = path.join(constants.OUTPUT_DIR, path.basename(file));

  // if src ends in a wildcard, copy everything matching the wildcard
  if (src.endsWith('*')) {
    const srcDir = path.dirname(src);
    const pattern = path.basename(src);
    const files = fs.readdirSync(srcDir);
    for (const f of files) {
      if (f.match(pattern)) {
        const srcFile = path.join(srcDir, f);
        const destFile = path.join(path.dirname(dest), path.basename(f));
        fs.copyFileSync(srcFile, destFile);
      }
    }
  } else {
    fs.copyFileSync(src, dest);
  }
}

function checkMinVersion(version: string, minVersion: string) {
  const versionParts = version.split('.');
  const minVersionParts = minVersion.split('.');
  for (let i = 0; i < minVersionParts.length; i++) {
    const part = parseInt(versionParts[i]);
    const minPart = parseInt(minVersionParts[i]);
    if (part < minPart) {
      return false;
    }
  }
  return true;
}

async function checkBuildTools() {
  // cargo
  try {
    await execGet(`cargo --version`);
  } catch (e) {
    console.log(e);
    console.error(
      'cargo not found. Please ensure Rust is installed and cargo is in your path.',
    );
    return false;
  }

  // rustc
  try {
    let rustcVersion = await execGet(`rustc --version`);
    rustcVersion = rustcVersion.split(' ')[1];
    console.log(`Found rustc ${rustcVersion}`);
    if (!checkMinVersion(rustcVersion, constants.RUST_MIN_VERSION)) {
      console.error(
        `Rust nightly ${constants.RUST_MIN_VERSION} or higher is required (found ${rustcVersion}). Please install the latest Rust nightly toolchain.`,
      );
      return false;
    }

    if (!rustcVersion.endsWith('-nightly')) {
      console.error(
        `Rust nightly is required. Please switch to a nightly toolchain.`,
      );
      return false;
    }
  } catch (e) {
    console.error(
      'rustc not found. Please ensure Rust is installed and rustc is in your path.',
    );
    return false;
  }

  // .NET
  try {
    const dotnetVersion = await execGet(`dotnet --version`);
    console.log(`Found .NET ${dotnetVersion}`);
    if (!checkMinVersion(dotnetVersion, constants.DOTNET_MIN_VERSION)) {
      console.error(
        `.NET 7 or higher is required (found ${dotnetVersion}). Please install the latest .NET SDK.`,
      );
      return false;
    }
  } catch (e) {
    console.error(
      `.NET 7 or higher is required. Please install the latest .NET SDK.`,
    );
    return false;
  }

  // check for pnpm
  try {
    await execGet(`pnpm --version`);
  } catch (e) {
    console.error('pnpm not found. Please install pnpm.');
    return false;
  }

  return true;
}

async function ensureArtifacts() {
  const result = await withProjects((name, meta) => {
    if (!meta.required || !meta.artifact) {
      return true;
    }

    if (!fs.existsSync(meta.artifact)) {
      console.error(`${name} artifact not found. Please execute:`);
      console.error(`  boneless build ${name}`);
      return false;
    }

    return true;
  });

  return result.filter((x) => !x).length === 0;
}

async function showHelp() {
  const terms = [
    'janky',
    'hacky',
    'shitty',
    'half-assed',
    'half-baked',
    'organic',
    'artisanal',
    'tasty',
    'undercooked',
  ];
  const term = terms[Math.floor(Math.random() * terms.length)];

  console.log();
  console.log(`Boneless: the ${term} Grebuloff build system`);
  console.log('--------------------------------------------');
  console.log('Build usage:\tboneless <build task> [...targets, default all]');
  console.log('Build tasks:');
  console.log('  clean\t\tClean build artifacts');
  console.log('  build\t\tBuild the project');
  console.log('  rebuild\tClean and rebuild the project');
  console.log('Targets for build tasks:');
  console.log('  all\t\tBuild everything (default)');
  await withProjects((name, meta) => {
    const tabs = name.length < 7 ? '\t\t' : '\t';
    console.log(`  ${name}${tabs}Build ${meta.description ?? name}`);
  });
  console.log();
  console.log('Run usage:\tboneless <run task> [options]');
  console.log('Run tasks:');
  console.log(
    '  set-path\tSets the path to ffxiv_dx11.exe in ~/.bonelessrc.json. Run this first!',
  );
  console.log('  launch\tFake-launch the game and inject Grebuloff');
  console.log(
    '  inject\tInject Grebuloff into a running game (must have ACLs modified)',
  );
}

//
// Parse args
//
const args = process.argv.slice(2);
let operation = args.shift()?.toLowerCase() ?? null;

// easter egg ops, for fun
switch (operation) {
  case 'wings':
    operation = 'clean';
    break;
  case 'chicken':
    operation = 'build';
    break;
  case 'beef':
    operation = 'rebuild';
    break;
}

// real-ish ops
let opType = null;
switch (operation) {
  case 'clean':
  case 'build':
  case 'rebuild':
    opType = 'build';
    break;
  case 'set-path':
  case 'launch':
  case 'inject':
    opType = 'run';
    break;
  default:
    console.error(`Unknown operation ${operation}`);
  // fallthrough
  case null:
    await showHelp();
    process.exit(1);
}

//
// Here we go...
//
if (opType === 'build') {
  // check build tools
  if (!(await checkBuildTools())) {
    process.exit(2);
  }

  // collect targets
  let targets: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const target = args[i].toLowerCase();

    if (target === 'all') {
      targets = Object.keys(constants.PROJECTS);
      break;
    }

    if (targets.includes(target)) {
      continue;
    }

    if (!(target in constants.PROJECTS)) {
      console.error(`Unknown target ${target}`);
      showHelp();
      process.exit(1);
    }

    targets.push(target);
  }

  if (targets.length === 0) {
    // collect `required` targets
    await withProjects((name, meta) => {
      if (meta.required) {
        targets.push(name);
      }
    });
  }

  //
  // Clean
  //
  if (operation === 'clean' || operation === 'rebuild') {
    console.log('Cleaning build artifacts...');

    if (fs.existsSync(constants.OUTPUT_DIR)) {
      fs.rmSync(constants.OUTPUT_DIR, { recursive: true });
    }

    await withProjects(async (name, meta) => {
      if (meta.type === 'js') {
        await exec(`npm run clean`, { cwd: meta.dir });
      } else if (meta.type === 'rust') {
        await exec(`cargo clean`, { cwd: meta.dir });
      }
    });

    await exec(`cargo clean`, { cwd: constants.CS_RUST_DIR });
  }

  //
  // Build
  //
  if (operation === 'build' || operation === 'rebuild') {
    // ensure clientstructs is cloned
    if (!fs.existsSync(constants.CS_RUST_DIR)) {
      console.log('Updating submodules...');
      await exec(`git submodule update --init --recursive`);
    }

    // build components
    await withProjects(async (name, meta) => {
      switch (meta.type) {
        case 'js':
          console.log(`Building JS project: ${name}...`);
          await execFor(name, 'pnpm install && pnpm run build');
          break;
        case 'rust':
          console.log(`Building Rust project: ${name}...`);
          await execFor(name, 'cargo build');
          if (meta.runTests) {
            console.log(`Running tests for ${name}...`);
            try {
              await execFor(name, 'cargo test');
            } catch (e) {
              if (meta.allowTestFailures) {
                console.warn(
                  `Tests failed for ${name}, but allowTestFailures is set. Continuing...`,
                );
              } else {
                console.error(`Tests failed for ${name}`);
                process.exit(4);
              }
            }
          }

          await copyArtifact(path.join('build', 'debug', 'grebuloff*'));
          await copyArtifact(
            path.join('build', 'x86_64-pc-windows-msvc', 'debug', 'grebuloff*'),
          );
          break;
        case 'dotnet':
          console.log(`Building .NET project: ${name}...`);
          await execFor(name, 'dotnet build');
          // todo: copy artifacts
          break;
        default:
          console.error(`Unknown project type ${meta.type} for ${name}`);
          process.exit(420);
      }
    }, targets);
  }
} else if (opType === 'run') {
  if (operation === 'set-path') {
    let gamePath = args.shift();
    if (!gamePath) {
      console.error('Missing path argument');
      process.exit(3);
    }

    // check to see if the path is a directory, and if so, append the exe
    try {
      if (fs.statSync(gamePath).isDirectory()) {
        gamePath = path.join(gamePath, 'ffxiv_dx11.exe');
      }
    } catch (e) {
      console.error(`Path ${gamePath} does not exist`);
      process.exit(3);
    }

    const config = {
      gamePath,
    };

    fs.writeFileSync(constants.RC_FILE, JSON.stringify(config));
    console.log(`Game path set to ${gamePath}`);
  } else if (operation === 'launch') {
    let config;
    try {
      config = JSON.parse(fs.readFileSync(constants.RC_FILE, 'utf8'));
      if (!config.gamePath) {
        throw 'deez';
      }
    } catch (e) {
      console.error(
        'Game path not set. Run `boneless set-path <path-to-ffxiv_dx11.exe>` first.',
      );
      process.exit(3);
    }

    const gamePath = config.gamePath;
    if (!fs.existsSync(gamePath)) {
      console.error(`Game executable not found at ${gamePath}`);
      process.exit(3);
    }

    // ensure the injector and runtime are built
    if (!(await ensureArtifacts())) {
      process.exit(4);
    }

    // launch the injector
    await exec(
      `${constants.PROJECTS.injector.artifact} launch --game-path "${gamePath}"`,
    );
  } else if (operation === 'inject') {
    // ensure the injector and runtime are built
    if (!(await ensureArtifacts())) {
      process.exit(4);
    }

    // launch the injector
    await exec(`${constants.PROJECTS.injector.artifact} inject`);
  }
}