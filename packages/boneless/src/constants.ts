import path from 'path';
import { fileURLToPath } from 'url';

export const WORKSPACE = path.resolve(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '../../../../'),
);

// Versions
export const DOTNET_MIN_VERSION = '7';
export const RUST_MIN_VERSION = '1.65';

// Files and directories
export const USER_HOME =
  process.env['HOME'] || process.env['USERPROFILE'] || '.';
export const RC_FILE = path.join(USER_HOME, '.bonelessrc.json');

export const CRATES_DIR = path.join(WORKSPACE, 'crates');
export const PACKAGES_DIR = path.join(WORKSPACE, 'packages');
export const OUTPUT_DIR = path.join(WORKSPACE, 'build');

export const CS_RUST_DIR = path.join(
  WORKSPACE,
  'deps',
  'FFXIVClientStructs',
  'rust',
);

export enum ProjectType {
  Rust = 'rust',
  JS = 'js',
  Dotnet = 'dotnet',
}

export interface ProjectMeta {
  type: ProjectType;
  description?: string;
  dir: string;
  artifact?: string;
  required?: boolean;
  runTests?: boolean;
  allowTestFailures?: boolean;
}

// Project info
export const PROJECTS: { [name: string]: ProjectMeta } = {
  cs: {
    type: ProjectType.Rust,
    description: 'FFXIVClientStructs Rust bindings',
    dir: CS_RUST_DIR,
    required: true,
    runTests: true,
    // allow failures for now until we fix up the bindings further
    allowTestFailures: true,
  },
  injector: {
    type: ProjectType.Rust,
    dir: path.join(CRATES_DIR, 'injector'),
    artifact: path.join(OUTPUT_DIR, 'grebuloff-injector.exe'),
    required: true,
  },
  llrt: {
    type: ProjectType.Rust,
    description: 'Low-Level Runtime (LLRT)',
    dir: path.join(CRATES_DIR, 'llrt'),
    artifact: path.join(OUTPUT_DIR, 'grebuloff_llrt.dll'),
    required: true,
  },
  hlrt: {
    type: ProjectType.JS,
    description: 'High-Level Runtime (hlrt)',
    dir: path.join(PACKAGES_DIR, 'hlrt'),
    artifact: path.join(OUTPUT_DIR, 'hlrt'),
    required: true,
  },
  boneless: {
    type: ProjectType.JS,
    description: 'boneless build tool',
    dir: path.join(PACKAGES_DIR, 'boneless'),
  },
  dalamud: {
    type: ProjectType.Dotnet,
    description: 'Dalamud helper plugin',
    dir: path.join(WORKSPACE, 'dalamud'),
  },
};
