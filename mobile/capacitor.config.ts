import { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'com.trueledger.app',
  appName: 'TrueLedger',
  webDir: 'dist',
  bundledWebRuntime: false,
  plugins: {
    // Rust core will be exposed via UniFFI-generated bindings
    RustPlugin: {
      path: '../core'
    }
  }
};

export default config;