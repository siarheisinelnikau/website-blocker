// Compatibility entry point. The current suite is always headless and uses disposable data.
if (process.argv.some(arg => ["--visual", "--managed"].includes(arg))) {
  throw new Error("The v0.4-only fixture modes were retired. Use Browser-Standalone.mjs Chrome|Edge for current checks; it temporarily backs up/restores the native-host registration.");
}
await import("./Browser-Standalone.mjs");
