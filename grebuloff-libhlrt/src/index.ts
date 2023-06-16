import console from './console.js';

((globalThis) => {
  // @ts-expect-error - initialization function, immediately removed after init
  globalThis.__init_libhlrt = () => {
    Deno.core.initializeAsyncOps();

    // @ts-expect-error
    delete globalThis.__init_libhlrt;
  };
  
  globalThis.console = console;
})(globalThis);
