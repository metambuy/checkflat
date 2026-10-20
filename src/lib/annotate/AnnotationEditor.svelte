<script lang="ts">
  // Full-screen photo annotation editor (Sprint 4, M6): the picture fitted to the screen, an SVG
  // overlay, three tools (ellipse, arrow, freehand) drawn with one pointer — touch on the phone,
  // mouse on Windows — a tap (movement only, any duration) selects a shape, Undo and Delete. Done
  // hands back the annotation (null when nothing is drawn); Cancel, Android Back and Escape
  // discard. The rules live in editor.ts; this component owns the DOM: pointer capture on the
  // picture wrapper, coordinates → image pixels, the back handler, Escape kept away from the sheet.
  import { onMount, tick } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { t } from "../i18n.svelte";
  import { pushBackHandler } from "../nav.svelte";
  import { begin, cancelStroke, commit, deleteSelected, end, initial, liveShape, setTool, THIN_CSS, undo, update, type EditorState, type Tool } from "./editor";
  import { fitBox, toImage, type Size } from "./geometry";
  import type { Annotation } from "./model";
  import ShapesOverlay from "./ShapesOverlay.svelte";

  let {
    path,
    annotation,
    ondone,
    oncancel,
  }: { path: string; annotation: Annotation | null; ondone: (a: Annotation | null) => void; oncancel: () => void } = $props();

  let root = $state<HTMLDivElement | null>(null);
  let stage = $state<HTMLDivElement | null>(null);
  let stageSize = $state<Size>({ w: 0, h: 0 });
  let natural = $state<Size | null>(null);
  let editor = $state<EditorState | null>(null);
  const box = $derived(natural ? fitBox(stageSize, natural) : null);
  const shapes = $derived.by(() => {
    if (!editor) return [];
    const live = liveShape(editor);
    return live ? [...editor.shapes, live] : editor.shapes;
  });
  /** Image pixels per CSS pixel at the current fit (tolerances are given in CSS px). */
  const pxPerCss = $derived(box && natural && box.w > 0 ? natural.w / box.w : 1);

  // The one pointer that is drawing, and where it came down (CSS px), for the tap/drag rule.
  let active: number | null = null;
  let down = { x: 0, y: 0 };

  onMount(() => {
    // Taking the focus closes the soft keyboard left by the description field and routes Escape here.
    void tick().then(() => root?.focus());
    const ro = new ResizeObserver(() => {
      if (stage) stageSize = { w: stage.clientWidth, h: stage.clientHeight };
    });
    if (stage) {
      ro.observe(stage);
      stageSize = { w: stage.clientWidth, h: stage.clientHeight };
    }
    const pop = pushBackHandler(() => {
      oncancel();
      return true;
    });
    return () => {
      ro.disconnect();
      pop();
    };
  });

  function loaded(e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    natural = { w: img.naturalWidth, h: img.naturalHeight };
    editor = initial(natural, annotation);
  }

  const imagePoint = (e: PointerEvent, el: HTMLElement) => {
    const r = el.getBoundingClientRect();
    return toImage(e.clientX - r.left, e.clientY - r.top, { x: 0, y: 0, w: r.width, h: r.height }, natural!);
  };

  function pointerdown(e: PointerEvent) {
    if (!editor || !natural) return;
    if (!e.isPrimary || (e.pointerType === "mouse" && e.button !== 0)) {
      // A second finger (or another button) ends the stroke in progress instead of joining it.
      if (active !== null) {
        editor = cancelStroke(editor);
        active = null;
      }
      return;
    }
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    active = e.pointerId;
    down = { x: e.clientX, y: e.clientY };
    editor = begin(editor, imagePoint(e, el));
  }
  function pointermove(e: PointerEvent) {
    if (e.pointerId !== active || !editor || !natural) return;
    editor = update(editor, imagePoint(e, e.currentTarget as HTMLElement), Math.hypot(e.clientX - down.x, e.clientY - down.y), THIN_CSS * pxPerCss);
  }
  function pointerup(e: PointerEvent) {
    if (e.pointerId !== active || !editor) return;
    active = null;
    editor = e.type === "pointerup" ? end(editor, pxPerCss) : cancelStroke(editor);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    // Handled here and stopped: the sheet under this editor cancels itself on the same key.
    e.preventDefault();
    e.stopPropagation();
    oncancel();
  }

  const tools: Tool[] = ["ellipse", "arrow", "freehand"];
</script>

<div class="editor" bind:this={root} role="dialog" aria-modal="true" aria-label={t("annotate.title")} tabindex="-1" onkeydown={keydown} oncontextmenu={(e) => e.preventDefault()}>
  <div class="top">
    <button type="button" class="dark" onclick={oncancel}>{t("common.cancel")}</button>
    <span class="grow title">{t("annotate.title")}</span>
    <button type="button" class="primary" onclick={() => ondone(editor ? commit(editor) : annotation)}>{t("annotate.done")}</button>
  </div>
  <div class="stage" bind:this={stage}>
    <!-- The wrapper is the picture's box: pointer events land here, the image and the overlay only show. -->
    <div
      class="picture"
      role="application"
      aria-label={t("annotate.hint")}
      class:ready={box !== null && box.w > 0}
      style={box ? `left:${box.x}px;top:${box.y}px;width:${box.w}px;height:${box.h}px` : ""}
      onpointerdown={pointerdown}
      onpointermove={pointermove}
      onpointerup={pointerup}
      onpointercancel={pointerup}
    >
      <img src={convertFileSrc(path)} alt="" draggable="false" onload={loaded} />
      {#if natural}
        <ShapesOverlay {shapes} width={natural.w} height={natural.h} selected={editor?.selected ?? null} />
      {/if}
    </div>
  </div>
  <!-- The limit message takes the hint's place (no layout jump) until the next stroke or a removal. -->
  <p class="hint" class:limit={editor?.limit} aria-live="polite">{editor?.limit ? t("annotate.limit") : t("annotate.hint")}</p>
  <div class="tools">
    {#each tools as tool (tool)}
      <button type="button" class="dark" class:on={editor?.tool === tool} aria-pressed={editor?.tool === tool} onclick={() => (editor = editor && setTool(editor, tool))} disabled={!editor}>
        {t(`annotate.${tool}`)}
      </button>
    {/each}
    <span class="gap"></span>
    <button type="button" class="dark" onclick={() => (editor = editor && undo(editor))} disabled={!editor || editor.shapes.length === 0}>{t("annotate.undo")}</button>
    <button type="button" class="dark" onclick={() => (editor = editor && deleteSelected(editor))} disabled={!editor || editor.selected === null}>{t("annotate.delete")}</button>
  </div>
</div>

<style>
  .editor {
    position: fixed; inset: 0; z-index: 20; background: #111; color: #fff; display: flex; flex-direction: column;
    padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
    outline: none;
  }
  .top { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 12px; }
  .title { font-weight: 600; text-align: center; }
  .stage { flex: 1; min-height: 0; position: relative; overflow: hidden; }
  .picture {
    position: absolute; visibility: hidden; touch-action: none; user-select: none; -webkit-user-select: none;
    -webkit-touch-callout: none; cursor: crosshair;
  }
  .picture.ready { visibility: visible; }
  .picture img { display: block; width: 100%; height: 100%; pointer-events: none; }
  .hint { margin: 0; padding: 0.25rem 12px 0; font-size: 0.85rem; color: #bbb; text-align: center; }
  .hint.limit { color: #f0b8a8; }
  .tools { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; padding: 0.5rem 12px; }
  .gap { flex: 1; }
  .dark { background: #2b2b2b; color: #fff; border-color: #555; }
  .dark:active { background: #444; }
  .dark.on { background: #e60000; border-color: #e60000; }
  .dark:disabled { opacity: 0.4; }
</style>
