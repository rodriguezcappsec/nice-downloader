import { open } from "@tauri-apps/plugin-dialog";
import {
  ChevronDown,
  Clock3,
  Download,
  FolderOpen,
  Link2,
  LoaderCircle,
  Music2,
  Search,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Select } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { useAppStore } from "../../stores/app-store";
import type { DownloadOptions } from "../../types";

export function NewDownloadPage() {
  const analysis = useAppStore((state) => state.analysis);
  const analyzing = useAppStore((state) => state.analyzing);
  const analyze = useAppStore((state) => state.analyze);
  const clearAnalysis = useAppStore((state) => state.clearAnalysis);
  const defaultDestination = useAppStore((state) => state.defaultDestination);
  const startDownload = useAppStore((state) => state.startDownload);
  const [url, setUrl] = useState("");
  const [qualityId, setQualityId] = useState("best");
  const [container, setContainer] = useState<DownloadOptions["container"]>("mp4");
  const [destination, setDestination] = useState(defaultDestination);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [embedMetadata, setEmbedMetadata] = useState(true);
  const [embedThumbnail, setEmbedThumbnail] = useState(false);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (!destination && defaultDestination) setDestination(defaultDestination);
  }, [defaultDestination, destination]);

  useEffect(() => {
    if (analysis) setQualityId(analysis.qualityOptions[0]?.id ?? "best");
  }, [analysis]);

  const selectedQuality = useMemo(
    () => analysis?.qualityOptions.find((quality) => quality.id === qualityId),
    [analysis, qualityId],
  );

  const audioOnly = selectedQuality?.kind === "audioOnly";

  useEffect(() => {
    if (audioOnly && !["mp3", "m4a", "opus"].includes(container)) setContainer("mp3");
    if (!audioOnly && ["mp3", "m4a", "opus"].includes(container)) setContainer("mp4");
  }, [audioOnly, container]);

  async function pasteUrl() {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        setUrl(text.trim());
        if (analysis) clearAnalysis();
      }
    } catch {
      // Clipboard permission is controlled by the desktop environment.
    }
  }

  async function chooseDestination() {
    const selected = await open({ directory: true, multiple: false, defaultPath: destination || undefined });
    if (typeof selected === "string") setDestination(selected);
  }

  async function submitDownload() {
    if (!analysis || !selectedQuality || !destination) return;
    setStarting(true);
    const options: DownloadOptions = {
      quality: selectedQuality,
      container,
      videoCodecPreference: "any",
      audioQuality: "best",
      audioOnly,
      subtitles: false,
      subtitleLanguages: [],
      embedThumbnail,
      embedMetadata,
      chapters: false,
      sponsorblock: false,
      destination,
      filenameTemplate: "%(title).180B [%(id)s].%(ext)s",
    };
    await startDownload({ url: analysis.webpageUrl || url, analysis, options });
    setStarting(false);
  }

  return (
    <div className="mx-auto w-full max-w-6xl px-8 py-7">
      <header className="mb-7">
        <h1 className="text-xl font-semibold tracking-tight">New Download</h1>
        <p className="mt-1 text-sm text-muted">Paste a media URL to inspect available formats before downloading.</p>
      </header>

      <section className="border border-line bg-panel p-5 shadow-panel">
        <label className="mb-2 block text-sm font-medium" htmlFor="media-url">
          Media URL
        </label>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Link2 className="absolute left-3.5 top-3.5 text-muted" size={17} />
            <Input
              id="media-url"
              className="pl-10"
              value={url}
              onChange={(event) => {
                setUrl(event.target.value);
                if (analysis) clearAnalysis();
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && url.trim() && !analyzing) void analyze(url);
              }}
              placeholder="https://www.youtube.com/watch?v=…"
              spellCheck={false}
            />
          </div>
          <Button variant="secondary" onClick={() => void pasteUrl()}>
            Paste
          </Button>
          <Button disabled={!url.trim() || analyzing} onClick={() => void analyze(url)}>
            {analyzing ? <LoaderCircle className="animate-spin" size={17} /> : <Search size={17} />}
            {analyzing ? "Analyzing" : "Analyze"}
          </Button>
        </div>
        <p className="mt-2 text-xs text-muted">Supports yt-dlp compatible sites. Analysis runs locally.</p>
      </section>

      {analyzing ? (
        <section className="mt-5 flex items-center gap-4 border border-line bg-panel p-6">
          <LoaderCircle className="animate-spin text-blue-400" size={24} />
          <div>
            <h2 className="text-sm font-semibold">Inspecting media</h2>
            <p className="mt-1 text-xs text-muted">yt-dlp is retrieving structured metadata and available streams…</p>
          </div>
        </section>
      ) : null}

      {analysis && !analyzing ? (
        <div className="mt-5 grid gap-5 xl:grid-cols-[minmax(0,1.1fr)_minmax(330px,.9fr)]">
          <section className="min-w-0 border border-line bg-panel">
            <div className="aspect-video overflow-hidden bg-black/25">
              {analysis.thumbnail ? (
                <img className="h-full w-full object-cover" src={analysis.thumbnail} alt="" />
              ) : (
                <div className="grid h-full place-items-center text-muted"><Download size={38} /></div>
              )}
            </div>
            <div className="p-5">
              <div className="mb-2 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-blue-400">
                <span>{analysis.siteName}</span>
                {analysis.durationSeconds != null ? (
                  <><span className="text-line">•</span><Clock3 size={13} /><span>{formatDuration(analysis.durationSeconds)}</span></>
                ) : null}
              </div>
              <h2 className="line-clamp-2 text-base font-semibold leading-6">{analysis.title}</h2>
              <p className="mt-2 text-sm text-muted">{analysis.uploader || analysis.extractor}</p>
              <div className="mt-4 flex flex-wrap gap-2 text-xs text-muted">
                <span className="border border-line bg-elevated px-2 py-1">{analysis.formats.length} streams detected</span>
                {largestKnownSize(analysis.formats) ? (
                  <span className="border border-line bg-elevated px-2 py-1">Up to ~{formatBytes(largestKnownSize(analysis.formats))}</span>
                ) : null}
              </div>
            </div>
          </section>

          <section className="border border-line bg-panel p-5">
            <h2 className="mb-5 text-sm font-semibold">Download options</h2>
            <div className="grid gap-4">
              <Field label="Quality">
                <Select value={qualityId} onChange={(event) => setQualityId(event.target.value)}>
                  {analysis.qualityOptions.map((quality) => (
                    <option key={quality.id} value={quality.id}>{quality.label} — {quality.description}</option>
                  ))}
                </Select>
              </Field>

              <Field label="Container">
                <Select value={container} onChange={(event) => setContainer(event.target.value as DownloadOptions["container"])}>
                  {(audioOnly ? ["mp3", "m4a", "opus"] : ["mp4", "mkv", "webm"]).map((format) => (
                    <option key={format} value={format}>{format.toUpperCase()}</option>
                  ))}
                </Select>
              </Field>

              <Field label="Destination" hint="The backend validates this directory before starting the engine.">
                <div className="flex gap-2">
                  <Input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} />
                  <Button variant="secondary" className="px-3" onClick={() => void chooseDestination()} aria-label="Choose destination folder">
                    <FolderOpen size={17} />
                  </Button>
                </div>
              </Field>

              <div className="border-t border-line pt-2">
                <button
                  className="flex w-full items-center justify-between py-2 text-sm font-medium text-muted hover:text-ink"
                  onClick={() => setAdvancedOpen((openState) => !openState)}
                >
                  Advanced options
                  <ChevronDown size={16} className={`transition-transform ${advancedOpen ? "rotate-180" : ""}`} />
                </button>
                {advancedOpen ? (
                  <div className="grid gap-3 pb-2 pt-2 text-sm text-muted">
                    <label className="flex items-center gap-3">
                      <input type="checkbox" checked={embedMetadata} onChange={(event) => setEmbedMetadata(event.target.checked)} />
                      Embed media metadata
                    </label>
                    <label className="flex items-center gap-3">
                      <input type="checkbox" checked={embedThumbnail} onChange={(event) => setEmbedThumbnail(event.target.checked)} />
                      Embed thumbnail
                    </label>
                  </div>
                ) : null}
              </div>

              <Button className="mt-1 w-full" disabled={!destination || starting} onClick={() => void submitDownload()}>
                {starting ? <LoaderCircle className="animate-spin" size={17} /> : audioOnly ? <Music2 size={17} /> : <Download size={17} />}
                {starting ? "Adding to queue" : "Download"}
              </Button>
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}

function largestKnownSize(formats: { approximateFileSize?: number | null }[]): number | null {
  const sizes = formats.flatMap((format) => format.approximateFileSize ?? []);
  return sizes.length ? Math.max(...sizes) : null;
}
