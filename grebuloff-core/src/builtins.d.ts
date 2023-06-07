declare namespace Deno {
    interface Core {
        print: (msg: string) => void;
        ops: {
            op_log: (msg: string) => void;
        }
    }

    const core: Core;
}