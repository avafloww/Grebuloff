import console from './console.js';

((globalThis) => {
  const boot = () => {
    // should not be called before a snapshot!
    Deno.core.initializeAsyncOps();

    console.trace('Hello world from console.trace()!');
    console.debug('Hello world from console.debug()!');
    console.info('Hello world from console.info()!');
    console.warn('Hello world from console.warn()!');
    console.error('Hello world from console.error()!');
    console.log('Hello world from console.log()!');
  };

  globalThis.console = console;

  boot();
})(globalThis);
