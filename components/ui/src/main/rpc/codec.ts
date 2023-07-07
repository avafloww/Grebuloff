import { Packr, Unpackr } from 'msgpackr';
import { Transform } from 'stream';
import { RpcMessageType } from './messages';

export class RpcDecoderStream extends Transform {
  private readonly codec: Unpackr;
  private incompleteBuffer: Buffer | null = null;

  constructor() {
    super({
      objectMode: true,
    });

    this.codec = new Unpackr();
  }

  _transform(chunk: Buffer, encoding: string, callback: () => void) {
    if (this.incompleteBuffer) {
      chunk = Buffer.concat([this.incompleteBuffer, chunk]);
      this.incompleteBuffer = null;
    }

    // read a little-endian 32-bit integer from the start of the chunk
    const length = chunk.readUInt32LE(0);
    if (chunk.length >= length + 4) {
      // if there's anything left in the chunk, it's the start of the next message - save it
      if (chunk.length > length + 4) {
        this.incompleteBuffer = chunk.subarray(length + 4);
      }

      // we have a complete message, trim the chunk to size
      const fullChunk = chunk.subarray(4, length + 4);

      // decode the message
      const decoded = this.codec.decode(fullChunk);

      // extract the message type
      const type = Object.keys(decoded.Ui)[0] as RpcMessageType;

      // push the decoded message
      this.push(new PackedRpcMessage(type, decoded.Ui[type]));
    }

    callback();
  }
}

export class RpcEncoderStream extends Transform {
  private readonly codec: Packr;

  constructor() {
    super({
      writableObjectMode: true,
    });

    this.codec = new Packr({ useRecords: false });
  }

  _transform(chunk: PackedRpcMessage, encoding: string, callback: () => void) {
    // encode the message
    const encoded = this.codec.encode(chunk.into());

    // prepend the length
    const length = Buffer.alloc(4);
    length.writeUInt32LE(encoded.length, 0);

    // push the encoded message
    this.push(Buffer.concat([length, encoded]));

    callback();
  }
}

export class PackedRpcMessage {
  constructor(
    public readonly type: RpcMessageType,
    public readonly data: unknown,
  ) {}

  into() {
    return {
      Ui: {
        [this.type.toString()]: this.data,
      },
    };
  }
}
