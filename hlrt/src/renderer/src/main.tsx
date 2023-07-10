import React from 'react';
import ReactDOM from 'react-dom/client';
import './assets/index.css';
import App from './App';

declare global {
  interface Window {
    grebuloffUiMode: 'pipe' | 'no-pipe';
  }
}

if (window.grebuloffUiMode === 'no-pipe') {
  document.body.classList.add('no-pipe');
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
