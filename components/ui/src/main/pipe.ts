// Communicates with LLRT via a named pipe.
// The named pipe ID is passed in as a command line argument.
// The pipe is created by the LLRT process.
// Whenever we get paint events, we send the raw bitmap data to the pipe.

import { BrowserWindow, NativeImage, Rectangle } from 'electron';
import * as net from 'net';

const createLengthMagic = (length: number) => {
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
    ...toInt32(length),
  ]);
};

const toInt32 = (num: number) => {
  return new Uint8Array([
    num & 0xff,
    (num >> 8) & 0xff,
    (num >> 16) & 0xff,
    (num >> 24) & 0xff,
  ]);
};

class PipeManager {
  private pipeName: string;
  private client: net.Socket | null = null;
  private latestImage: PipeImageData | null = null;

  constructor(pipeId: string, window: BrowserWindow) {
    this.pipeName = `\\\\.\\pipe\\grebuloff-llrt-ui-${pipeId}`;

    window.webContents.on('paint', this.onPaint.bind(this));
    window.webContents.setFrameRate(60);
  }

  connect() {
    console.log(`connecting to LLRT on ${this.pipeName}`);

    // connect to the pipe
    this.client = net.connect(
      {
        path: this.pipeName,
      },
      this.onConnect.bind(this),
    );
  }

  private onConnect() {
    if (!this.client) {
      throw new Error('client is null');
    }

    this.client.on('data', this.onData.bind(this));
    this.client.on('drain', this.onDrain.bind(this));
    this.client.on('end', this.onDisconnect.bind(this));

    console.log('connected to LLRT pipe');

    this.onDrain();
  }

  private onDisconnect() {
    console.log('disconnected from LLRT pipe');
  }

  private onData(data: Buffer) {
    console.log('received data from LLRT pipe');
    console.dir(data);
  }

  private onDrain() {
    // send the next image, if we have one
    if (this.client && this.latestImage) {
      this.latestImage.write(this.client);
      this.latestImage = null;
    } else {
      // check back to see when conditions allow
      setTimeout(this.onDrain.bind(this), 50);
    }
  }

  private onPaint(
    _event: Electron.Event,
    dirty: Electron.Rectangle,
    image: Electron.NativeImage,
  ) {
    const pipeImage = new PipeImageData(dirty, image);

    this.latestImage = pipeImage;
  }
}

class PipeImageData {
  constructor(private dirty: Rectangle, private image: NativeImage) {
    // force the dirty rect to be the full image for now
    this.dirty = {
      x: 0,
      y: 0,
      width: image.getSize().width,
      height: image.getSize().height,
    };
  }

  write(client: net.Socket) {
    // 32 bytes of additional image header data
    // LLRT can compute image buffer size from length in magic, minus these 32 bytes
    const imgHeader = new Uint8Array([
      // message type: "UI:IMG"
      0x55,
      0x49,
      0x3a,
      0x49,
      0x4d,
      0x47,
      // null terminator
      0x00,
      // full image width (4 bytes)
      ...toInt32(this.image.getSize().width),
      // full image height (4 bytes)
      ...toInt32(this.image.getSize().height),
      // bytes per pixel (1 byte)
      0x04,
      // dirty rect x (4 bytes)
      ...toInt32(this.dirty.x),
      // dirty rect y (4 bytes)
      ...toInt32(this.dirty.y),
      // dirty rect width (4 bytes)
      ...toInt32(this.dirty.width),
      // dirty rect height (4 bytes)
      ...toInt32(this.dirty.height),
    ]);

    // the rest of the buffer consists only of the dirty region's bitmap data
    // or well, it's supposed to... but it's actually the full image data, and trimming
    // it results in some weird artifacts. so we'll just send the full image data for now.
    const imageBuffer = this.image.getBitmap();

    const lengthMagic = createLengthMagic(
      imgHeader.length + imageBuffer.length,
    );

    // client.write(Buffer.concat([lengthMagic, imgHeader, imageBuffer]));
    client.write(lengthMagic);
    client.write(imgHeader);
    client.write(imageBuffer);
  }
}

export default PipeManager;
