import { BrowserWindow, NativeImage, Rectangle } from 'electron';
import { RpcClient } from './rpc/client';
import { RpcMessageType } from './rpc/messages';

export class UiPainter {
  private paintData?: PaintData;
  private shouldRepaint = true;
  private sending = false;

  constructor(private rpc: RpcClient, browser: BrowserWindow) {
    browser.webContents.beginFrameSubscription(true, this.onPaint.bind(this));
    setInterval(this.tick.bind(this), 1);
  }

  private tick() {
    this.repaint();
  }

  async repaint(): Promise<boolean> {
    if (!this.paintData) return false;

    if (this.rpc.connected && !this.sending && this.shouldRepaint) {
      this.sending = true;

      this.shouldRepaint = false;
      await this.rpc.send(RpcMessageType.Paint, this.paintData?.prepared);

      this.sending = false;
      return true;
    }

    return false;
  }

  private onPaint(image: NativeImage, dirty: Rectangle) {
    this.paintData = new PaintData(dirty, image);
    this.shouldRepaint = true;
  }
}

export enum ImageFormat {
  RGBA8 = 'RGBA8',
}

export class PaintData {
  constructor(
    public readonly dirty: Rectangle,
    public readonly image: NativeImage,
  ) {}

  /**
   * Gets the prepared data to send to LLRT.
   * You must consume this object in the same event loop tick as using this getter;
   * otherwise, the image data is not guaranteed to be valid.
   */
  get prepared() {
    const size = this.image.getSize();
    return {
      vw: size.width,
      vh: size.height,
      f: ImageFormat.RGBA8.toString(),
      dx: this.dirty.x,
      dy: this.dirty.y,
      dw: this.dirty.width,
      dh: this.dirty.height,
      d: this.image.getBitmap(),
    };
  }
}
