import { Type } from 'class-transformer';

export enum RpcMessageType {
  Resize = 'Resize',
}

export class RpcMessageResize {}

export class PackedRpcMessage {
  public readonly type: RpcMessageType;

  @Type(() => Object, {
    discriminator: {
      property: 'type',
      subTypes: [{ value: RpcMessageResize, name: RpcMessageType.Resize }],
    },
  })
  public readonly data: any;

  constructor(type: RpcMessageType, data: any) {
    this.type = type;
    this.data = data;
  }

  into() {
    return {
      Ui: {
        [this.type.toString()]: this.data,
      },
    };
  }
}
