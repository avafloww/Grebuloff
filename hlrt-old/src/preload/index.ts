import { contextBridge } from 'electron';

contextBridge.exposeInMainWorld(
  'grebuloffUiMode',
  process.env['SHOW_NO_PIPE'] ? 'no-pipe' : 'pipe',
);
