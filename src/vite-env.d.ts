/// <reference types="vite/client" />
interface ImportMetaEnv {
  /** "1" shows the Dev (spike) screen in a release build. Never set in CI. */
  readonly VITE_SPIKES?: string;
}

// pdfjs-dist ships a declaration for the legacy entry point only.
declare module "pdfjs-dist/build/pdf.mjs" {
  export * from "pdfjs-dist";
}
