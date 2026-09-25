/// <reference types="vite/client" />
interface ImportMetaEnv {
  /** "1" shows the Dev (spike) screen in a release build. Never set in CI. */
  readonly VITE_SPIKES?: string;
}
