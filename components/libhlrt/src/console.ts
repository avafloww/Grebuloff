function parseArgs(...args: any[]) {
  return args.map(arg => {
    if (typeof arg === 'object') {
      return JSON.stringify(arg);
    }

    return arg;
  }).join(' ');
}

export enum LogLevel {
  Trace = -2,
  Debug = -1,
  Info = 0,
  Warn = 1,
  Error = 2
}

const console: Console = {
  // Logging facilities
  trace: (...args: any[]) => Deno.core.ops.op_log(parseArgs(args), LogLevel.Trace),
  debug: (...args: any[]) => Deno.core.ops.op_log(parseArgs(args), LogLevel.Debug),
  info: (...args: any[]) => Deno.core.ops.op_log(parseArgs(args), LogLevel.Info),
  warn: (...args: any[]) => Deno.core.ops.op_log(parseArgs(args), LogLevel.Warn),
  error: (...args: any[]) => Deno.core.ops.op_log(parseArgs(args), LogLevel.Error),
  log: (...args: any[]) => Deno.core.ops.op_log(parseArgs(args), LogLevel.Info),

  // TODO
  assert: (condition: boolean | undefined, data: any): void => {},
  clear: (): void => {},
  count: (label: string | undefined): void => {},
  countReset: (label: string | undefined): void => {},
  dir: (item?: any, options?: any): void => {},
  dirxml: (...data: any[]): void => {},
  group: (...data: any[]): void => {},
  groupCollapsed: (...data: any[]): void => {},
  groupEnd: (): void => {},
  table: (tabularData: any, properties: string[] | undefined): void => {},
  time: (label: string | undefined): void => {},
  timeEnd: (label: string | undefined): void => {},
  timeLog: (label: string | undefined, data: any): void => {},
  timeStamp: (label: string | undefined): void => {},
};

export default console;
