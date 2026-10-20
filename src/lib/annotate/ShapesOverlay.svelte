<script lang="ts">
  // The shapes of one photo as an SVG over its <img>. The viewBox is the image's natural size,
  // so the paths are in image pixels and scale with the picture; `fit` matches the image's
  // object-fit (contain in the editor, cover on thumbnails) so the overlay lines up with it.
  // `thumb` draws a fixed 1.5 px stroke (the real one would be under a pixel at 96 px).
  import { arrowHead } from "./geometry";
  import { ARROW_HEAD, COLOUR, strokeWidth, type Shape } from "./model";

  let {
    shapes,
    width,
    height,
    fit = "contain",
    selected = null,
    thumb = false,
  }: { shapes: Shape[]; width: number; height: number; fit?: "contain" | "cover"; selected?: number | null; thumb?: boolean } = $props();

  const stroke = $derived(strokeWidth(width, height));
  const f = (v: number) => Math.round(v * 10) / 10;

  function d(s: Shape): string {
    switch (s.t) {
      case "ellipse": {
        const [cx, cy, rx, ry] = [s.cx * width, s.cy * height, s.rx * width, s.ry * height];
        return `M${f(cx - rx)},${f(cy)} A${f(rx)},${f(ry)} 0 1 0 ${f(cx + rx)},${f(cy)} A${f(rx)},${f(ry)} 0 1 0 ${f(cx - rx)},${f(cy)} Z`;
      }
      case "arrow": {
        const tail: [number, number] = [s.x1 * width, s.y1 * height];
        const tip: [number, number] = [s.x2 * width, s.y2 * height];
        const legs = arrowHead(tail, tip, ARROW_HEAD * stroke);
        const shaft = `M${f(tail[0])},${f(tail[1])} L${f(tip[0])},${f(tip[1])}`;
        return legs ? `${shaft} M${f(legs[0][0])},${f(legs[0][1])} L${f(tip[0])},${f(tip[1])} L${f(legs[1][0])},${f(legs[1][1])}` : shaft;
      }
      case "freehand":
        return s.pts.map(([x, y], i) => `${i === 0 ? "M" : "L"}${f(x * width)},${f(y * height)}`).join(" ");
    }
  }
</script>

{#if width > 0 && height > 0}
  <svg class="overlay" viewBox="0 0 {width} {height}" preserveAspectRatio="xMidYMid {fit === 'cover' ? 'slice' : 'meet'}" aria-hidden="true">
    {#each shapes as s, i (i)}
      {#if i === selected}
        <path d={d(s)} class="halo" stroke-width={stroke * 3} />
      {/if}
      <path d={d(s)} stroke={COLOUR} stroke-width={thumb ? 1.5 : stroke} vector-effect={thumb ? "non-scaling-stroke" : undefined} />
    {/each}
  </svg>
{/if}

<style>
  .overlay { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; overflow: visible; }
  path { fill: none; stroke-linecap: round; stroke-linejoin: round; }
  .halo { stroke: #fff; opacity: 0.85; }
</style>
