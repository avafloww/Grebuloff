import { Packr, Unpackr } from 'msgpackr';
import { Transform, TransformCallback } from 'stream';
import { RpcMessageType } from './messages';

abstract class LengthDecoderStream extends Transform {
  private incompleteChunk: Buffer | null = null;

  constructor() {
    super({
      objectMode: true,
    });
  }

  readFullChunk(chunk: Buffer): Buffer | null {
    if (this.incompleteChunk) {
      chunk = Buffer.concat([this.incompleteChunk, chunk]);
      this.incompleteChunk = null;
    }

    // read a little-endian 32-bit integer from the start of the chunk
    const length = chunk.readUInt32LE(0);
    if (chunk.length >= length + 4) {
      // if there's anything left in the chunk, it's the start of the next message - save it
      if (chunk.length > length + 4) {
        this.incompleteChunk = chunk.subarray(length + 4);
      }

      // we have a complete chunk, trim the chunk to size and return it
      return chunk.subarray(4, length + 4);
    }

    return null;
  }
}

abstract class LengthEncoderStream extends Transform {
  constructor() {
    super({
      writableObjectMode: true,
    });
  }

  writeFullChunk(chunk: Buffer) {
    // prepend the length
    const length = Buffer.alloc(4);
    length.writeUInt32LE(chunk.length, 0);

    // push the encoded message
    this.push(Buffer.concat([length, chunk]));
  }
}

export class RpcMessageDecoderStream extends LengthDecoderStream {
  private readonly codec: Unpackr;

  constructor() {
    super();
    this.codec = new Unpackr();
  }

  _transform(
    partialChunk: Buffer,
    encoding: string,
    callback: TransformCallback,
  ) {
    const fullChunk = this.readFullChunk(partialChunk);
    if (fullChunk) {
      // optimization: if the first byte is < 0xDE or > 0xDF, then we know it's not a valid
      // msgpack structure for our purposes (since we only use maps), so we can skip the
      // deserialization step and treat it as a raw message
      // currently, the UI doesn't _receive_ any raw messages, but this is here for completeness
      if (fullChunk[0] < 0xde || fullChunk[0] > 0xdf) {
        this.push(fullChunk);
      } else {
        // decode the message
        const decoded = this.codec.decode(fullChunk);

        // extract the message type
        const type = Object.keys(decoded.Ui)[0] as RpcMessageType;

        // push the decoded message
        this.push(new PackedRpcMessage(type, decoded.Ui[type]));
      }
    }

    callback();
  }
}

export class RpcMessageEncoderStream extends LengthEncoderStream {
  private readonly codec: Packr;

  constructor() {
    super();
    this.codec = new Packr({ useRecords: false });
  }

  _transform(
    message: PackedRpcMessage,
    encoding: string,
    callback: () => void,
  ) {
    const encoded = this.codec.encode(message.into());
    this.writeFullChunk(encoded);

    callback();
  }
}

export class RpcRawEncoderStream extends LengthEncoderStream {
  _transform(message: Buffer, encoding: string, callback: () => void) {
    this.writeFullChunk(message);

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
