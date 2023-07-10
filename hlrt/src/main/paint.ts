import { BrowserWindow, NativeImage, Rectangle } from 'electron';
import { RpcClient } from './rpc/client';

export class UiPainter {
  private paintData?: PaintData;
  private shouldRepaint = true;
  private sending = false;

  constructor(private rpc: RpcClient, private browser: BrowserWindow) {
    browser.webContents.on('paint', this.onPaint.bind(this));
    setInterval(this.tick.bind(this), 1);
  }

  handleResize(width: number, height: number) {
    console.log(`resize: ${width}x${height}`);
    this.paintData = undefined;
    this.browser.setContentSize(width, height);
  }

  async repaint(): Promise<boolean> {
    if (!this.paintData) return false;

    if (this.rpc.ready && !this.sending && this.shouldRepaint) {
      this.sending = true;

      this.shouldRepaint = false;
      await this.rpc.sendRaw(this.paintData.prepareBuffer());

      this.sending = false;
      return true;
    }

    return false;
  }

  private tick() {
    this.repaint();
  }

  private onPaint(_event: Event, dirty: Rectangle, image: NativeImage) {
    this.paintData = new PaintData(dirty, image);
    this.shouldRepaint = true;
  }
}

export enum ImageFormat {
  BGRA8 = 0,
}

export class PaintData {
  constructor(
    public readonly dirty: Rectangle,
    public readonly image: NativeImage,
  ) {}

  /**
   * Gets the prepared buffer to send to LLRT.
   * You must consume this buffer in the same event loop tick as calling this method;
   * otherwise, the image data is not guaranteed to be valid.
   */
  prepareBuffer(): Buffer {
    const buf = Buffer.alloc(5);

    const size = this.image.getSize();
    buf.writeUInt8(ImageFormat.BGRA8, 0);
    buf.writeUInt16LE(size.width, 1);
    buf.writeUInt16LE(size.height, 3);

    return Buffer.concat([buf, this.image.getBitmap()]);
  }
}
