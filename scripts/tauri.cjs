// Tauri's launcher expects a conventional node executable name. Normalize
// arguments so packaged Electron/Node runtimes can invoke the same CLI safely.
const args = process.argv.slice(2);
process.argv.splice(0, process.argv.length, 'node', 'tauri', ...args);
require('../node_modules/@tauri-apps/cli/tauri.js');
