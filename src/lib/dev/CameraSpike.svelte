<script lang="ts">
  // Spike B. Part 1: plain <input capture>. Part 2: native camera intent via the camera-capture plugin.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { StagedPhoto } from "../api";
  import { captureStagedPhoto, pickStagedPhoto } from "../photoSource";
  let log = $state<string[]>([]);
  let nativeUrl = $state<string | null>(null);
  let nativeInfo = $state("");
  let nativeBusy = $state(false);

  // Production path: plugin (camera or Photo Picker) → cache file → core `stage_photo` → asset URL.
  async function nativePhoto(source: () => Promise<StagedPhoto | null>, label: string) {
    nativeBusy = true;
    add(`${label} called`);
    try {
      const r = await source();
      if (!r) { add(`${label}: cancelled`); return; }
      nativeUrl = convertFileSrc(r.path);
      nativeInfo = `${r.takenAt} · token ${r.token.slice(0, 8)}`;
      add(`${label}: staged ${nativeInfo}`);
    } catch (e) {
      add(`${label} error: ${JSON.stringify(e)}`);
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
  <p class="muted">Part 2: Kotlin plugin (camera intent / Photo Picker) → Rust resize, orientation, EXIF time (Sprint 3b).</p>
  <button onclick={() => nativePhoto(captureStagedPhoto, "capture")} disabled={nativeBusy}>Camera (native plugin)</button>
  <button onclick={() => nativePhoto(pickStagedPhoto, "pick")} disabled={nativeBusy}>Gallery (Photo Picker)</button>
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
