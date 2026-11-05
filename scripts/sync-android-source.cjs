// Keep maintained Android adapters separate from the machine-specific Tauri
// project. Every normal Android development/build command refreshes the copy.
const { cpSync, existsSync } = require('node:fs');
const { resolve } = require('node:path');

const source = resolve(__dirname, '../src-tauri/android/app/src/main');
const destination = resolve(__dirname, '../src-tauri/gen/android/app/src/main');
if (!existsSync(destination)) {
  throw new Error('Android project not found. Run npm run android:init first.');
}
cpSync(source, destination, { recursive: true });
console.log('Android native source synchronized.');
