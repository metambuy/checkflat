<script lang="ts">
  // The observation list (M7): the project's open observations, newest first (the report's order),
  // each with its first photo (and the shapes drawn on it), ref, fraction, plan and the first line of
  // the description. A fraction filter narrows it; a tap opens that plan with the pin selected and
  // centred (PlanScreen `focusId`). The route carries the filter and the scroll position, so Back
  // from a plan opened out of the list returns here as it was left (routes.ts, D-027). The list
  // scrolls in its own box (the shared <main> scrolls every screen) with fixed-height rows, so a
  // restored scrollTop lands on the same rows.
  import { onMount, tick, untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { api, asAppError, type AppError, type Fraction, type ObservationRow } from "../api";
  import { t } from "../i18n.svelte";
  import { back, go } from "../nav.svelte";
  import type { ObservationsScreen } from "../routes";
  import { descriptionStart, filterRows, fractionChips, planFromList } from "../list/rows";
  import ShapesOverlay from "../annotate/ShapesOverlay.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  let { route }: { route: ObservationsScreen } = $props();

  let rows = $state<ObservationRow[]>([]);
  let fractions = $state<Fraction[]>([]);
  let filter = $state<string | null>(untrack(() => route.filter));
  let loaded = $state(false);
  let error = $state<AppError | null>(null);
  let scroller = $state<HTMLDivElement | null>(null);
  /** Natural sizes of the thumbnails' images, for their overlays. */
  let sizes = $state<Record<string, { w: number; h: number }>>({});

  const chips = $derived(fractionChips(fractions.map((f) => f.code), rows));
  const shown = $derived(filterRows(rows, filter));

  onMount(() => {
    (async () => {
      try {
        [rows, fractions] = await Promise.all([api.listObservations(route.projectId), api.listFractions(route.projectId)]);
        // A remembered filter whose fraction is gone would show nothing and no chip to leave it.
        if (filter !== null && !chips.includes(filter)) filter = null;
        loaded = true;
        await tick();
        if (scroller) scroller.scrollTop = route.scrollTop;
      } catch (e) {
        error = asAppError(e);
        loaded = true;
      }
    })();
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !e.defaultPrevented) back();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  function sized(id: string, e: Event) {
    const img = e.currentTarget as HTMLImageElement;
    sizes = { ...sizes, [id]: { w: img.naturalWidth, h: img.naturalHeight } };
  }

  /** Open the row's plan on its pin; Back returns here with this filter and scroll position. */
  function open(row: ObservationRow) {
    go(planFromList(route, row, filter, scroller?.scrollTop ?? 0));
  }
</script>

<div class="list-screen">
  <div class="topbar">
    <button class="plain back" onclick={() => back()}>← {t("nav.back")}</button>
    <strong class="grow title">{t("observations.title")}</strong>
    {#if loaded}<span class="muted">{shown.length === 1 ? t("observations.count_one") : t("observations.count", { count: shown.length })}</span>{/if}
  </div>
  <ErrorBanner {error} ondismiss={() => (error = null)} />

  {#if chips.length > 0}
    <div class="chips" role="group" aria-label={t("observations.fractions")}>
      <button type="button" class="chip" class:on={filter === null} aria-pressed={filter === null} onclick={() => (filter = null)}>{t("observations.all")}</button>
      {#each chips as code (code)}
        <button type="button" class="chip" class:on={filter === code} aria-pressed={filter === code} onclick={() => (filter = code)}>{code}</button>
      {/each}
    </div>
  {/if}

  <div class="scroller" bind:this={scroller}>
    {#if !loaded}
      <p class="muted pad">{t("common.loading")}</p>
    {:else if rows.length === 0}
      <p class="muted pad">{t("observations.empty")}</p>
    {:else if shown.length === 0}
      <p class="muted pad">{t("observations.filter_empty")}</p>
    {:else}
      <ul class="rows">
        {#each shown as row (row.id)}
          <li>
            <button type="button" class="row" onclick={() => open(row)}>
              <span class="thumb">
                {#if row.thumbPath}
                  <img src={convertFileSrc(row.thumbPath)} alt="" loading="lazy" decoding="async" draggable="false" onload={(e) => sized(row.id, e)} />
                  {#if row.thumbAnnotation && sizes[row.id]}
                    <ShapesOverlay shapes={row.thumbAnnotation.shapes} width={sizes[row.id].w} height={sizes[row.id].h} fit="cover" thumb />
                  {/if}
                {:else}
                  <span class="nophoto">{t("observations.no_photo")}</span>
                {/if}
              </span>
              <span class="text">
                <span class="head"><strong class="ref">{row.ref}</strong>{#if row.fraction}<span class="frac">{row.fraction}</span>{/if}</span>
                <span class="plan">{row.planTitle}</span>
                <span class="desc">{descriptionStart(row.description)}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

<style>
  .list-screen { position: absolute; inset: 0; display: flex; flex-direction: column; }
  .topbar { display: flex; align-items: center; gap: 0.5rem; padding: 0.25rem 12px; background: #fff; border-bottom: 1px solid #ddd; }
  .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .topbar .back { justify-content: center; }
  .list-screen > :global(.banner) { margin: 0.4rem 12px; }
  .chips { display: flex; gap: 0.5rem; padding: 0.5rem 12px; overflow-x: auto; background: #fff; border-bottom: 1px solid #ddd; flex: none; }
  .chip { flex: none; border-radius: 22px; }
  .chip.on { background: #143c78; color: #fff; border-color: #143c78; }
  .scroller { flex: 1; min-height: 0; overflow-y: auto; padding-bottom: env(safe-area-inset-bottom); }
  .pad { padding: 0 16px; }
  .rows { list-style: none; margin: 0; padding: 0; }
  .rows li { border-bottom: 1px solid #e3e3e3; background: #fff; }
  /* Fixed height: the restored scroll position must land on the same rows whatever has loaded. */
  .row { display: flex; align-items: center; gap: 0.75rem; width: 100%; height: 76px; padding: 0 12px; border: none; border-radius: 0; background: none; text-align: left; }
  .row:active { background: #e6e6e6; }
  .thumb { position: relative; flex: none; width: 56px; height: 56px; border-radius: 6px; overflow: hidden; background: #e8e8e8; border: 1px solid #ccc; }
  .thumb img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .nophoto { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: 10px; color: #777; text-align: center; padding: 2px; }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; font-size: 14px; line-height: 1.25; }
  .head { display: flex; align-items: center; gap: 0.4rem; min-width: 0; }
  .ref { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .frac { flex: none; font-size: 11px; padding: 0 7px; border-radius: 10px; background: #e3ebf7; color: #143c78; }
  .plan { color: #666; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .desc { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
