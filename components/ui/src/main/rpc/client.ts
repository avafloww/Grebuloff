import { default as net, Socket } from 'net';
import EventEmitter from 'events';
import { RpcMessageType } from './messages';
import { PackedRpcMessage, RpcDecoderStream, RpcEncoderStream } from './codec';

export class RpcClient extends EventEmitter {
  private pipeName: string;
  private client: Socket | null = null;
  private encoder: RpcEncoderStream | null = null;
  private decoder: RpcDecoderStream | null = null;

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
        this.encoder.once('drain', () => {
          resolve();
        });
      }
    });
  }

  private onConnect() {
    if (!this.client) {
      throw new Error('client is null');
    }

    this.encoder = new RpcEncoderStream();
    this.decoder = new RpcDecoderStream();

    this.client.pipe(this.decoder);
    this.encoder.pipe(this.client);

    this.decoder.on('data', this.onData.bind(this));
    this.client.on('end', this.onDisconnect.bind(this));
    this.encoder.on('drain', this.onDrain.bind(this));

    console.log('connected to LLRT pipe');

    this.emit('connect');
  }

  private onDisconnect() {
    console.log('disconnected from LLRT pipe');
    this.emit('close');
  }

  private onData(data: PackedRpcMessage) {
    console.log('received data from LLRT pipe');
    console.dir(data);
  }

  private onDrain() {
    this.emit('drain');
  }
}
