// Communicates with LLRT via a named pipe.
// The named pipe ID is passed in as a command line argument.
// The pipe is created by the LLRT process.
// Whenever we get paint events, we send the raw bitmap data to the pipe.

import { BrowserWindow, NativeImage } from 'electron';
import * as net from 'net';

class PipeManager {
  private pipeName: string;
  private client: net.Socket | null = null;
  private isConnected = false;
  private latestImage: PipeImageData | null = null;

  constructor(pipeId: string, window: BrowserWindow) {
    this.pipeName = `\\\\.\\pipe\\grebuloff-llrt-ui-${pipeId}`;

    window.webContents.on('paint', this.onPaint.bind(this));
    window.webContents.setFrameRate(60);
  }

  connect() {
    console.log(`connecting to LLRT on ${this.pipeName}`);

    // connect to the pipe
    this.client = net.connect(this.pipeName, this.onConnect.bind(this));
  }

  private onConnect() {
    if (!this.client) {
      throw new Error('client is null');
    }

    this.client.on('data', this.onData.bind(this));
    this.client.on('end', this.onDisconnect.bind(this));

    console.log('connected to LLRT pipe');
    this.isConnected = true;

    // send the latest image, if we have one
    if (this.latestImage) {
      console.log('sending latest image');
      this.latestImage.write(this.client);
      this.latestImage = null;
    }
  }

  private onDisconnect() {
    console.log('disconnected from LLRT pipe');
    this.isConnected = false;
  }

  private onData(data: Buffer) {
    console.log('received data from LLRT pipe');
    console.dir(data);
  }

  private onPaint(
    _event: Electron.Event,
    _dirty: Electron.Rectangle,
    image: Electron.NativeImage,
  ) {
    const pipeImage = new PipeImageData(
      image.getSize().width,
      image.getSize().height,
      image,
    );

    if (this.client && this.isConnected) {
      // send the bitmap bytes to the pipe directly
      pipeImage.write(this.client);
    } else {
      // store the image until we connect
      this.latestImage = pipeImage;
    }
  }
}

class PipeImageData {
  constructor(
    private width: number,
    private height: number,
    private image: NativeImage,
  ) {}

  write(client: net.Socket) {
    // 16 bytes of additional image header data
    // LLRT can compute image buffer size from length in magic, minus these 16 bytes
    const imgHeader = new Uint8Array([
      // message type: "UI:IMG"
      0x55,
      0x49,
      0x3a,
      0x49,
      0x4d,
      0x47,
      // separator
      0x00,
      // width (4 bytes)
      ...this.toInt32(this.width),
      // height (4 bytes)
      ...this.toInt32(this.height),
      // bytes per pixel (1 byte)
      0x04,
    ]);

    const imageBuffer = this.image.getBitmap();

    const lengthMagic = this.createLengthMagic(
      imgHeader.length + imageBuffer.length,
    );

    client.write(Buffer.concat([lengthMagic, imgHeader, imageBuffer]));
  }

  private createLengthMagic(length: number) {
    return new Uint8Array([
      // magic: 0xffffffff + "LLRT"
      0xff,
      0xff,
      0xff,
      0xff,
      0x4c,
      0x4c,
      0x52,
      0x54,
      // remaining message length
      ...this.toInt32(length),
    ]);
  }

  private toInt32(num: number) {
    return new Uint8Array([
      num & 0xff,
      (num >> 8) & 0xff,
      (num >> 16) & 0xff,
      (num >> 24) & 0xff,
    ]);
  }
}

export default PipeManager;
