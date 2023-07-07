import { default as net, Socket } from 'net';
import EventEmitter from 'events';
import { RpcMessageType } from './messages';
import {
  PackedRpcMessage,
  RpcMessageDecoderStream,
  RpcMessageEncoderStream,
  RpcRawEncoderStream,
} from './codec';

export class RpcClient extends EventEmitter {
  private pipeName: string;
  private client: Socket | null = null;
  private encoder: RpcMessageEncoderStream | null = null;
  private rawEncoder: RpcRawEncoderStream | null = null;
  private decoder: RpcMessageDecoderStream | null = null;

  constructor(pipeId: string) {
    super();
    this.pipeName = `\\\\.\\pipe\\grebuloff-llrt-ui-${pipeId}`;
  }

  connect() {
    console.log(`connecting to LLRT on ${this.pipeName}`);

    this.client = net.connect(
      { path: this.pipeName },
      this.onConnect.bind(this),
    );
  }

  get connected() {
    return this.client && this.client.writable;
  }

  async send(type: RpcMessageType, data: unknown) {
    new Promise<void>((resolve, reject) => {
      if (!this.client || !this.encoder) {
        return reject(new Error('client is null'));
      }

      const packed = new PackedRpcMessage(type, data);
      if (this.encoder.write(packed)) {
        process.nextTick(resolve);
      } else {
        this.client.once('drain', () => {
          resolve();
        });
      }
    });
  }

  async sendRaw(data: Buffer) {
    new Promise<void>((resolve, reject) => {
      if (!this.client || !this.rawEncoder) {
        return reject(new Error('client is null'));
      }

      if (this.rawEncoder.write(data)) {
        process.nextTick(resolve);
      } else {
        this.client.once('drain', () => {
          resolve();
        });
      }
    });
  }

  private onConnect() {
    if (!this.client) {
      throw new Error('client is null');
    }

    this.encoder = new RpcMessageEncoderStream();
    this.rawEncoder = new RpcRawEncoderStream();
    this.decoder = new RpcMessageDecoderStream();

    this.client.pipe(this.decoder);
    this.encoder.pipe(this.client);
    this.rawEncoder.pipe(this.client);

    this.decoder.on('data', this.onData.bind(this));
    this.client.on('end', this.onDisconnect.bind(this));
    this.client.on('drain', this.onDrain.bind(this));

    console.log('connected to LLRT pipe');

    this.emit('connect');
  }

  private onDisconnect() {
    console.log('disconnected from LLRT pipe');
    this.emit('close');
  }

  private onData(data: PackedRpcMessage | Buffer) {
    console.log('received data from LLRT pipe');
    console.dir(data);

    if (!(data instanceof PackedRpcMessage)) {
      throw new Error('received raw data from LLRT pipe');
    }
  }

  private onDrain() {
    this.emit('drain');
  }
}
