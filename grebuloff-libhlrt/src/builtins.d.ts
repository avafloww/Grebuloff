import { LogLevel } from './console.js';

declare global {
  declare namespace Deno {
    interface Core {
      print: (msg: string) => void;
      ops: {
        op_log: (msg: string, level: LogLevel) => void;
      };

      initializeAsyncOps(): void;
    }

    const core: Core;
  }
}