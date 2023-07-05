import { app, BrowserWindow } from 'electron';
import { join } from 'path';
import { optimizer, is } from '@electron-toolkit/utils';
import PipeManager from './pipe';

// force a scale factor of 1, even on high-DPI displays, as we will control scaling ourselves
app.commandLine.appendSwitch('high-dpi-support', '1');
app.commandLine.appendSwitch('force-device-scale-factor', '1');

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.whenReady().then(() => {
  // Default open or close DevTools by F12 in development
  // and ignore CommandOrControl + R in production.
  // see https://github.com/alex8088/electron-toolkit/tree/master/packages/utils
  app.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window);
  });

  const showNoPipe = !!process.env['SHOW_NO_PIPE'];

  const mainWindow = new BrowserWindow({
    width: 1920,
    height: 1080,
    show: showNoPipe,
    title: 'Grebuloff UI Host',
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      sandbox: true,
      nodeIntegration: false,
      offscreen: !showNoPipe,
    },
  });

  mainWindow.webContents.setWindowOpenHandler((_details) => {
    // shell.openExternal(details.url);
    return { action: 'deny' };
  });

  // HMR for renderer base on electron-vite cli.
  // Load the remote URL for development or the local html file for production.
  if (is.dev && process.env['ELECTRON_RENDERER_URL']) {
    mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL']);
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'));
  }

  if (showNoPipe) {
    console.log('not connecting to pipe: SHOW_NO_PIPE is set');
    return;
  }

  const pipeId = process.env['LLRT_PIPE_ID'];
  if (!pipeId) {
    console.error('missing pipe id; set env var LLRT_PIPE_ID appropriately');
    process.exit(1);
  }

  console.log(`pipe id: ${pipeId}`);

  // create the pipe manager and connect
  const pipeManager = new PipeManager(pipeId, mainWindow);
  pipeManager.connect();
});

app.on('window-all-closed', () => {
  app.quit();
});