<script lang="ts">
  // The report action on the project screen (Sprint 6a, D-028): language chosen at generation, an
  // indeterminate "Generating report…" with the controls disabled, then the latest report (date,
  // language, pages, size) with Open and Share (Android; Windows saves it in Sprint 6b). The run
  // outlives this component (`jobs.ts`), so leaving the screen and coming back shows it again.
  import { onMount } from "svelte";
  import { api, asAppError, deviceTimeZone, utcOffsetMin, type AppError, type ReportInfo, type ReportResult } from "../api";
  import { lang, t, type Lang } from "../i18n.svelte";
  import { isAndroid } from "../platform";
  import { devlog, SPIKES } from "../devlog";
  import { formatDateTime, formatSize } from "./format";
  import { runningJob, startJob } from "./jobs";

  let { projectId, onerror }: { projectId: string; onerror: (e: AppError) => void } = $props();

  let reportLang = $state<Lang>(lang());
  let generating = $state(false);
  let latest = $state<ReportInfo | null>(null);
  let loaded = $state(false);
  let skippedPhotos = $state(0);
  /** Spike builds only (the D-026 measurement builds read the timings off the screen). */
  let timings = $state<string | null>(null);
  /** Open/Share in flight: the copy for the other app is being made. */
  let handing = $state(false);
  let android = $state(false);

  function follow(job: Promise<ReportResult>) {
    generating = true;
    job
      .then((r) => {
        latest = r.report;
        loaded = true;
        skippedPhotos = r.stats.skippedPhotos;
        const s = r.stats;
        const line = `report: ${r.report.pages} pages, ${r.report.bytes} B, total ${r.totalMs} ms · snapshots ${s.snapshotMs} ms · photos ${s.photoMs} ms · compile ${r.compileMs} ms · pdf ${r.pdfMs} ms · ${s.observations} obs · ${s.photos} photos · ${s.withoutSnapshot} without snapshot`;
        devlog(line);
        if (SPIKES) timings = line;
      })
      .catch((e) => onerror(asAppError(e)))
      .finally(() => (generating = false));
  }

  onMount(() => {
    void isAndroid().then((a) => (android = a));
    const job = runningJob<ReportResult>(projectId);
    if (job) follow(job);
    api
      .latestReport(projectId)
      .then((r) => {
        if (!loaded) latest = r; // a run that finished first has the newer one
        loaded = true;
      })
      .catch((e) => onerror(asAppError(e)));
  });

  function generate() {
    skippedPhotos = 0;
    timings = null;
    follow(startJob(projectId, () => api.generateReport(projectId, reportLang, deviceTimeZone(), utcOffsetMin())));
  }

  async function hand(action: "open" | "share") {
    handing = true;
    try {
      if (action === "open") await api.openReport(projectId);
      else await api.shareReport(projectId, t("report.share_title"));
    } catch (e) {
      const err = asAppError(e);
      if (err.code === "no_report") latest = null;
      onerror(err);
    } finally {
      handing = false;
    }
  }

  const pagesLabel = (n: number) => (n === 1 ? t("report.pages_one") : t("report.pages", { count: n }));
</script>

<section class="card" aria-labelledby="report-heading">
  <h2 id="report-heading">{t("report.section")}</h2>
  <div class="row">
    <label class="row">
      <span class="muted">{t("report.language")}</span>
      <select bind:value={reportLang} disabled={generating}>
        <option value="en">{t("lang.en")}</option>
        <option value="pt">{t("lang.pt")}</option>
      </select>
    </label>
    <button class="primary" onclick={generate} disabled={generating}>{t("report.generate")}</button>
  </div>
  {#if generating}
    <div class="progress" aria-live="polite">
      <progress aria-label={t("report.generating")}></progress>
      <span class="muted">{t("report.generating")}</span>
    </div>
  {/if}
  {#if latest}
    <p class="latest">
      {t("report.latest", {
        date: formatDateTime(new Date(latest.createdAt)),
        lang: t(latest.lang === "pt" ? "lang.pt" : "lang.en"),
        pages: pagesLabel(latest.pages),
        size: formatSize(latest.bytes, lang()),
      })}
    </p>
    {#if android}
      <div class="row">
        <button onclick={() => hand("open")} disabled={generating || handing}>{t("report.open")}</button>
        <button onclick={() => hand("share")} disabled={generating || handing}>{t("report.share")}</button>
      </div>
    {/if}
  {:else if loaded && !generating}
    <p class="muted">{t("report.none")}</p>
  {/if}
  {#if skippedPhotos > 0}<p class="muted">{t("report.skipped_photos", { count: skippedPhotos })}</p>{/if}
  {#if timings}<p class="muted timings"><code>{timings}</code></p>{/if}
</section>

<style>
  h2 { margin-top: 0; }
  .progress { display: flex; flex-direction: column; gap: 0.25rem; margin-top: 0.5rem; }
  progress { width: 100%; height: 6px; }
  .latest { margin: 0.5rem 0; }
  .timings { word-break: break-all; }
  .timings code { font-size: 11px; }
</style>
