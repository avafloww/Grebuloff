//
// Boneless: the janky Grebuloff build script
//

import child_process from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import readline from 'readline';
import events from 'events';

const __dirname = path.resolve(path.join(path.dirname(fileURLToPath(import.meta.url)), '..'));

// Versions
const DOTNET_MIN_VERSION = '7';
const RUST_MIN_VERSION = '1.65';

// Files and directories
const USER_HOME = process.env['HOME'] || process.env['USERPROFILE'] || '.';
const RC_FILE = path.join(USER_HOME, '.bonelessrc.json');

const COMPONENTS_DIR = path.join(__dirname, 'components');
const OUTPUT_DIR = path.join(__dirname, 'build');

const CS_DIR = path.join(__dirname, 'deps', 'FFXIVClientStructs');
const CS_RUST_DIR = path.join(CS_DIR, 'rust');
const CS_GENERATED_FILE = path.join(CS_RUST_DIR, 'lib', 'src', 'generated.rs');

const CS_EXPORTER_DIR = path.join(CS_RUST_DIR, 'exporter');
const CS_EXPORTER_BIN_DIR = path.join(CS_EXPORTER_DIR, 'bin', 'Debug', 'net7.0');
const CS_EXPORTER_BIN = path.join(CS_EXPORTER_BIN_DIR, 'RustExporter.exe');

// Project info
const PROJECTS = {
  injector: {
    type: 'rust',
    dir: path.join(COMPONENTS_DIR, 'injector'),
    artifact: path.join(OUTPUT_DIR, 'grebuloff-injector.exe'),
    required: true,
  },
  llrt: {
    type: 'rust',
    dir: path.join(COMPONENTS_DIR, 'llrt'),
    artifact: path.join(OUTPUT_DIR, 'grebuloff_llrt.dll'),
    required: true,
  },
  libhlrt: {
    type: 'js',
    dir: path.join(COMPONENTS_DIR, 'libhlrt'),
    artifact: path.join(OUTPUT_DIR, 'libhlrt'),
    required: true,
  },
  hlrt: {
    type: 'js',
    dir: path.join(COMPONENTS_DIR, 'hlrt'),
    artifact: path.join(OUTPUT_DIR, 'hlrt'),
    required: true,
  },
  ui: {
    type: 'js',
    dir: path.join(COMPONENTS_DIR, 'ui'),
  },
  dalamud: {
    type: 'dotnet',
    dir: path.join(COMPONENTS_DIR, 'dalamud'),
  }
};

//
// Utility functions
//
async function exec(cmd, extraOpts = {}) {
  return new Promise((resolve, reject) => {
    if (!extraOpts.silent) {
      console.log(`> ${cmd}`);
    }

    let child = child_process.spawn(cmd, Object.assign({ shell: true }, extraOpts));

    let output = '';

    function recv(data) {
      let str = data.toString();
      output += str;

      if (!extraOpts.silent) {
        process.stdout.write(str);
      }
    }

    child.stdout.on('data', recv);
    child.stderr.on('data', recv);

    child.on('close', (code) => {
      if (code === 0) {
        if (!extraOpts.silent) console.log(); // newline for cleanliness
        resolve(output);
      } else {
        reject(`Process exited with code ${code}`);
      }
    });
  });
}

async function withProjects(fn, projects = Object.keys(PROJECTS)) {
  if (typeof projects === 'string') {
    projects = [projects];
  }

  let ret = [];
  for (const p of projects) {
    const meta = PROJECTS[p];
    let pret = fn(p, meta);
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

async function execFor(project, cmd, extraOpts = {}) {
  await withProjects(async (name, meta) => {
    await exec(cmd, Object.assign({ cwd: meta.dir }, extraOpts));
  }, project);
}

async function copyArtifact(file) {
  if (!fs.existsSync(OUTPUT_DIR)) {
    fs.mkdirSync(OUTPUT_DIR);
  }

  const src = path.join(__dirname, file);
  const dest = path.join(OUTPUT_DIR, path.basename(file));

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

function checkMinVersion(version, minVersion) {
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
    await exec(`cargo --version`, { silent: true });
  } catch (e) {
    console.log(e);
    console.error('cargo not found. Please ensure Rust is installed and cargo is in your path.');
    return false;
  }

  // rustc
  try {
    let rustcVersion = await exec(`rustc --version`, { silent: true });
    rustcVersion = rustcVersion.split(' ')[1];
    console.log(`Found rustc ${rustcVersion}`);
    if (!checkMinVersion(rustcVersion, RUST_MIN_VERSION)) {
      console.error(`Rust nightly ${RUST_MIN_VERSION} or higher is required (found ${rustcVersion}). Please install the latest Rust nightly toolchain.`);
      return false;
    }

    if (!rustcVersion.endsWith('-nightly')) {
      console.error(`Rust nightly is required. Please switch to a nightly toolchain.`);
      return false;
    }
  } catch (e) {
    console.error('rustc not found. Please ensure Rust is installed and rustc is in your path.');
    return false;
  }

  // .NET
  try {
    const dotnetVersion = await exec(`dotnet --version`, { silent: true });
    console.log(`Found .NET ${dotnetVersion}`);
    if (!checkMinVersion(dotnetVersion, DOTNET_MIN_VERSION)) {
      console.error(`.NET 7 or higher is required (found ${dotnetVersion}). Please install the latest .NET SDK.`);
      return false;
    }
  } catch (e) {
    console.error(`.NET 7 or higher is required. Please install the latest .NET SDK.`);
    return false;
  }

  // check for pnpm
  try {
    await exec(`pnpm --version`, { silent: true });
  } catch (e) {
    console.error('pnpm not found. Please install pnpm.');
    return false;
  }

  return true;
}

async function shouldBuildClientStructs() {
  if (!fs.existsSync(CS_EXPORTER_BIN) || !fs.existsSync(CS_GENERATED_FILE)) {
    return true;
  }

  // check the first line only, generated.rs is huge...
  const rl = readline.createInterface({
    input: fs.createReadStream(CS_GENERATED_FILE),
    crlfDelay: Infinity,
  });

  const [line] = await events.once(rl, 'line');
  rl.close();

  // always rebuild if the rev line is missing
  if (!line.startsWith('// rev: ')) {
    return true;
  }

  const rev = line.substring(8);

  // always rebuild if the working tree was dirty when the file was generated
  if (rev.endsWith('-dirty')) {
    return true;
  }

  // otherwise, only rebuild if the CS rev is different
  const gitRev = await exec(`git describe --always`, { cwd: CS_DIR, silent: true });

  return rev.trim() !== gitRev.trim();
}

async function ensureArtifacts() {
  const result = await withProjects((name, meta) => {
    if (!meta.required || !meta.artifact) {
      return true;
    }

    if (!fs.existsSync(meta.artifact)) {
      console.error(`${project} artifact not found. Please execute:`);
      console.error(`  boneless build ${project}`);
      return false;
    }

    return true;
  });

  return result.filter(x => !x).length === 0;
}

function showHelp() {
  const terms = ['janky', 'hacky', 'shitty', 'half-assed', 'half-baked', 'organic', 'artisanal', 'tasty', 'undercooked'];
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
  withProjects((name, meta) => {
    const tabs = name.length < 7 ? '\t\t' : '\t';
    console.log(`  ${name}${tabs}Build ${project}`);
  });
  console.log();
  console.log('Run usage:\tboneless <run task> [options]');
  console.log('Run tasks:');
  console.log('  set-path\tSets the path to ffxiv_dx11.exe in ~/.bonelessrc.json. Run this first!');
  console.log('  launch\tFake-launch the game and inject Grebuloff');
  console.log('  inject\tInject Grebuloff into a running game (must have ACLs modified)');

  process.exit(1);
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
  case null:
    showHelp();
    process.exit(1);
}

//
// Here we go...
//
if (opType === 'build') {
  // check build tools
  if (!await checkBuildTools()) {
    process.exit(2);
  }

  // collect targets
  let targets = [];
  for (let i = 0; i < args.length; i++) {
    const target = args[i].toLowerCase();

    if (target === 'all') {
      targets = PROJECTS.keys();
      break;
    }

    if (targets.includes(target)) {
      continue;
    }

    if (!(target in PROJECTS)) {
      console.error(`Unknown target ${target}`);
      showHelp();
      process.exit(1);
    }

    targets.push(target);
  }

  if (targets.length === 0) {
    // collect `required` targets
    withProjects((name, meta) => {
      if (meta.required) {
        targets.push(name);
      }
    })
  }

  //
  // Clean
  //
  if (operation === 'clean' || operation === 'rebuild') {
    console.log('Cleaning build artifacts...');

    if (fs.existsSync(OUTPUT_DIR)) {
      fs.rmSync(OUTPUT_DIR, { recursive: true });
    }

    await withProjects(async (name, meta) => {
      if (meta.type === 'js') {
        await exec(`npm run clean`, { cwd: meta.dir });
      } else if (meta.type === 'rust') {
        await exec(`cargo clean`, { cwd: meta.dir });
      }
    });

    await exec(`cargo clean`, { cwd: CS_RUST_DIR });
    await exec(`dotnet clean`, { cwd: CS_EXPORTER_DIR });

    if (fs.existsSync(CS_GENERATED_FILE)) {
      fs.unlinkSync(CS_GENERATED_FILE);
    }
  }

  //
  // Build
  //
  if (operation === 'build' || operation === 'rebuild') {
    // ensure clientstructs is cloned
    if (!fs.existsSync(CS_RUST_DIR)) {
      console.log('Updating submodules...');
      await exec(`git submodule update --init --recursive`);
    }

    // ensure we have exported structs
    if (await shouldBuildClientStructs()) {
      console.log('Building FFXIVClientStructs/RustExporter...');
      await exec(`dotnet build -c Debug`, { cwd: CS_EXPORTER_DIR });

      console.log('Running FFXIVClientStructs/RustExporter...');
      await exec(CS_EXPORTER_BIN, { cwd: CS_EXPORTER_BIN_DIR });
    }

    // build components
    withProjects(async (name, meta) => {
      switch (meta.type) {
        case 'js':
          console.log(`Building JS project: ${name}...`);
          await execFor(name, 'pnpm install && pnpm run build');
          break;
        case 'rust':
          console.log(`Building Rust project: ${name}...`);
          await execFor(name, 'cargo build');
          await copyArtifact(path.join('target', 'debug', 'grebuloff*'));
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
    let path = args.shift();
    if (!path) {
      console.error('Missing path argument');
      process.exit(3);
    }

    // check to see if the path is a directory, and if so, append the exe
    try {
      if (fs.statSync(path).isDirectory()) {
        path = path.join(path, 'ffxiv_dx11.exe');
      }
    } catch (e) {
      console.error(`Path ${path} does not exist`);
      process.exit(3);
    }

    const config = {
      gamePath: path,
    };

    fs.writeFileSync(RC_FILE, JSON.stringify(config));
    console.log(`Game path set to ${path}`);
  } else if (operation === 'launch') {
    let config;
    try {
      config = JSON.parse(fs.readFileSync(RC_FILE, 'utf8'));
      if (!config.gamePath) {
        throw 'deez';
      }
    } catch (e) {
      console.error('Game path not set. Run `boneless set-path <path-to-ffxiv_dx11.exe>` first.');
      process.exit(3);
    }

    const gamePath = config.gamePath;
    if (!fs.existsSync(gamePath)) {
      console.error(`Game executable not found at ${gamePath}`);
      process.exit(3);
    }

    // ensure the injector and runtime are built
    if (!await ensureArtifacts()) {
      process.exit(4);
    }

    // launch the injector
    await exec(`${PROJECTS.injector.artifact} launch --game-path "${gamePath}"`);
  } else if (operation === 'inject') {
    // ensure the injector and runtime are built
    if (!await ensureArtifacts()) {
      process.exit(4);
    }

    // launch the injector
    await exec(`${PROJECTS.injector.artifact} inject`);
  }
}