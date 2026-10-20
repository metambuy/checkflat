<script module lang="ts">
  // 24×24 line icons drawn with `currentColor`. They carry no text: the button that shows one gives
  // itself an accessible name (aria-label and title from the i18n strings).
  const PATHS = {
    edit: "M12 20h9 M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z",
    move: "M5 9l-3 3 3 3 M9 5l3-3 3 3 M15 19l-3 3-3-3 M19 9l3 3-3 3 M2 12h20 M12 2v20",
    trash: "M3 6h18 M8 6V4h8v2 M19 6l-1 14H6L5 6 M10 11v6 M14 11v6",
    close: "M18 6L6 18 M6 6l12 12",
    list: "M8 6h13 M8 12h13 M8 18h13 M3 6h.01 M3 12h.01 M3 18h.01",
    ellipse: "M3 12a9 6 0 1 0 18 0a9 6 0 1 0 -18 0",
    arrow: "M5 19L19 5 M9 5h10v10",
    freehand: "M3 17c3-9 5 3 8-2s5-7 10-5",
    undo: "M9 14L4 9l5-5 M4 9h11a5 5 0 0 1 0 10h-4",
  };
  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 22 }: { name: IconName; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d={PATHS[name]} /></svg>

<style>
  svg { display: block; flex: none; }
</style>
