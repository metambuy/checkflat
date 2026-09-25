<script lang="ts">
  // Spike B. Part 1: plain <input capture>. Part 2: native camera intent via the camera-capture plugin.
  import { invoke } from "@tauri-apps/api/core";
  let log = $state<string[]>([]);
  let nativeUrl = $state<string | null>(null);
  let nativeInfo = $state("");
  let nativeBusy = $state(false);

  async function nativeCapture() {
    nativeBusy = true;
    add("plugin capture() called");
    try {
      const r = await invoke<{ path: string; bytes: number; base64: string }>("capture_photo");
      nativeUrl = `data:image/jpeg;base64,${r.base64}`;
      nativeInfo = `${r.path} · ${(r.bytes / 1024).toFixed(0)} KB (read in Rust)`;
      add(`plugin resolved: ${nativeInfo}`);
    } catch (e) {
      add(`plugin error: ${e}`);
    } finally {
      nativeBusy = false;
    }
  }
  let previewUrl = $state<string | null>(null);
  let info = $state("");

  const add = (s: string) => (log = [...log, `${new Date().toLocaleTimeString()} ${s}`]);

  function onChange(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const f = input.files?.[0];
    if (!f) { add("change fired with no file"); return; }
    if (previewUrl) URL.revokeObjectURL(previewUrl);
    previewUrl = URL.createObjectURL(f);
    info = `${f.name || "(no name)"} · ${f.type || "(no type)"} · ${(f.size / 1024).toFixed(0)} KB · lastModified ${new Date(f.lastModified).toISOString()}`;
    add(`file received: ${info}`);
  }
</script>

<section>
  <h2>Spike B — camera</h2>
  <p class="muted">Part 1: <code>&lt;input type="file" accept="image/*" capture="environment"&gt;</code>. Note whether the camera opens directly or a chooser appears.</p>
  <label class="btn">
    Take photo (input capture)
    <input type="file" accept="image/*" capture="environment" onchange={onChange} onclick={() => add("input clicked")} hidden />
  </label>
  <p class="muted">Part 2: Kotlin plugin → <code>MediaStore.ACTION_IMAGE_CAPTURE</code>, path returned to Rust.</p>
  <button onclick={nativeCapture} disabled={nativeBusy}>Take photo (native plugin)</button>
  {#if nativeUrl}
    <img src={nativeUrl} alt="native capture" />
    <p class="muted">{nativeInfo}</p>
  {/if}
  <p class="muted">userAgent: {navigator.userAgent}</p>
  {#if previewUrl}
    <img src={previewUrl} alt="captured" />
    <p class="muted">{info}</p>
  {/if}
  <pre>{log.join("\n")}</pre>
</section>

<style>
  section { padding: 0.75rem; overflow: auto; height: 100%; }
  .btn { display: inline-block; padding: 0.6rem 1rem; border: 1px solid #bbb; border-radius: 6px; background: #fff; }
  img { max-width: 100%; max-height: 40vh; display: block; margin-top: 0.5rem; border: 1px solid #ccc; }
</style>
