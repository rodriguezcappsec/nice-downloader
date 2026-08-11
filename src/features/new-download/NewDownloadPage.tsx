import { open } from "@tauri-apps/plugin-dialog";
import {
  ChevronDown,
  Clock3,
  Download,
  Film,
  FolderOpen,
  Link2,
  LoaderCircle,
  ListOrdered,
  Music2,
  Search,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Select, Textarea } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { useAppStore } from "../../stores/app-store";
import type { DownloadOptions, GalleryItem, MediaAnalysis, StartDownloadRequest } from "../../types";

export function NewDownloadPage() {
  const analysis = useAppStore((state) => state.analysis);
  const analyzing = useAppStore((state) => state.analyzing);
  const batchAnalysis = useAppStore((state) => state.batchAnalysis);
  const galleryItems = useAppStore((state) => state.galleryItems);
  const galleryItemsLoading = useAppStore((state) => state.galleryItemsLoading);
  const analyze = useAppStore((state) => state.analyze);
  const analyzeBatch = useAppStore((state) => state.analyzeBatch);
  const analyzePlaylist = useAppStore((state) => state.analyzePlaylist);
  const loadGalleryItems = useAppStore((state) => state.loadGalleryItems);
  const clearAnalysis = useAppStore((state) => state.clearAnalysis);
  const defaultDestination = useAppStore((state) => state.defaultDestination);
  const startDownload = useAppStore((state) => state.startDownload);
  const startDownloads = useAppStore((state) => state.startDownloads);
  const [url, setUrl] = useState("");
  const [qualityId, setQualityId] = useState("best");
  const [container, setContainer] = useState<DownloadOptions["container"]>("mp4");
  const [destination, setDestination] = useState(defaultDestination);
  const [outputConflictPolicy, setOutputConflictPolicy] = useState<DownloadOptions["outputConflictPolicy"]>("skip");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [embedMetadata, setEmbedMetadata] = useState(true);
  const [embedThumbnail, setEmbedThumbnail] = useState(false);
  const [starting, setStarting] = useState(false);
  const [selectedBatchUrls, setSelectedBatchUrls] = useState<string[]>([]);
  const [selectedGalleryIndices, setSelectedGalleryIndices] = useState<number[]>([]);
  const [gallerySearch, setGallerySearch] = useState("");

  useEffect(() => {
    if (!destination && defaultDestination) setDestination(defaultDestination);
  }, [defaultDestination, destination]);

  useEffect(() => {
    if (analysis) setQualityId(analysis.qualityOptions[0]?.id ?? "best");
  }, [analysis]);

  useEffect(() => {
    setSelectedBatchUrls(
      batchAnalysis?.items.filter((item) => item.analysis).map((item) => item.inputUrl) ?? [],
    );
  }, [batchAnalysis]);

  useEffect(() => {
    if (!galleryItems) {
      setSelectedGalleryIndices([]);
      setGallerySearch("");
      return;
    }
    setSelectedGalleryIndices((selected) => [
      ...new Set([...selected, ...galleryItems.items.map((item) => item.index)]),
    ]);
  }, [galleryItems]);

  const selectedQuality = useMemo(
    () => analysis?.qualityOptions.find((quality) => quality.id === qualityId),
    [analysis, qualityId],
  );

  const audioOnly = selectedQuality?.kind === "audioOnly";
  const isGallery = analysis?.engine === "galleryDl";
  const filteredGalleryItems = useMemo(() => {
    const query = gallerySearch.trim().toLocaleLowerCase();
    if (!query) return galleryItems?.items ?? [];
    return galleryItems?.items.filter((item) =>
      `${item.filename}.${item.extension || ""}`.toLocaleLowerCase().includes(query),
    ) ?? [];
  }, [galleryItems, gallerySearch]);
  const selectedGalleryItems = useMemo(
    () => galleryItems?.items.filter((item) => selectedGalleryIndices.includes(item.index)) ?? [],
    [galleryItems, selectedGalleryIndices],
  );
  const selectedGallerySize = selectedGalleryItems.reduce((total, item) => total + (item.sizeBytes ?? 0), 0);

  useEffect(() => {
    if (audioOnly && !["mp3", "m4a", "opus"].includes(container)) setContainer("mp3");
    if (!audioOnly && ["mp3", "m4a", "opus"].includes(container)) setContainer("mp4");
  }, [audioOnly, container]);

  async function pasteUrl() {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        setUrl(text.trim());
        if (analysis || batchAnalysis) clearAnalysis();
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
      outputConflictPolicy,
      gallerySelection: isGallery && galleryItems
        ? selectedGalleryItems.map((item) => ({ index: item.index, sizeBytes: item.sizeBytes }))
        : null,
    };
    await startDownload({ url: analysis.downloadUrl || analysis.webpageUrl || url, analysis, options });
    setStarting(false);
  }

  async function submitBatch() {
    if (!batchAnalysis || !destination) return;
    const requests = batchAnalysis.items.flatMap((item): StartDownloadRequest[] => {
      if (!item.analysis || !selectedBatchUrls.includes(item.inputUrl)) return [];
      return [{
        url: item.analysis.downloadUrl || item.analysis.webpageUrl || item.inputUrl,
        analysis: item.analysis,
        options: defaultBatchOptions(item.analysis, destination, outputConflictPolicy),
      }];
    });
    if (requests.length === 0) return;
    setStarting(true);
    await startDownloads(requests);
    setStarting(false);
  }

  const inputUrls = parseInputUrls(url);

  return (
    <div className="mx-auto w-full max-w-6xl px-8 py-7">
      <header className="mb-7">
        <h1 className="text-xl font-semibold tracking-tight">New Download</h1>
        <p className="mt-1 text-sm text-muted">Paste a media or gallery URL to inspect it before downloading.</p>
      </header>

      <section className="border border-line bg-panel p-5 shadow-panel">
        <label className="mb-2 block text-sm font-medium" htmlFor="media-url">
          Media URL
        </label>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Link2 className="absolute left-3.5 top-3.5 text-muted" size={17} />
            <Textarea
              id="media-url"
              className="min-h-11 py-3 pl-10"
              value={url}
              onChange={(event) => {
                setUrl(event.target.value);
                if (analysis || batchAnalysis) clearAnalysis();
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey && inputUrls.length === 1 && !analyzing) {
                  event.preventDefault();
                  void analyze(inputUrls[0]);
                }
              }}
              placeholder="YouTube, Reddit, Bunkr album, or another supported URL"
              spellCheck={false}
            />
          </div>
          <Button variant="secondary" onClick={() => void pasteUrl()}>
            Paste
          </Button>
          <Button variant="secondary" disabled={inputUrls.length !== 1 || analyzing} onClick={() => void analyzePlaylist(inputUrls[0])}>
            <ListOrdered size={17} /> Playlist
          </Button>
          <Button disabled={inputUrls.length === 0 || analyzing} onClick={() => inputUrls.length > 1 ? void analyzeBatch(inputUrls) : void analyze(inputUrls[0])}>
            {analyzing ? <LoaderCircle className="animate-spin" size={17} /> : <Search size={17} />}
            {analyzing ? "Analyzing" : inputUrls.length > 1 ? `Analyze ${inputUrls.length}` : "Analyze"}
          </Button>
        </div>
        <p className="mt-2 text-xs text-muted">Supports yt-dlp compatible sites and Bunkr albums through gallery-dl. Analysis runs locally.</p>
      </section>

      {analyzing ? (
        <section className="mt-5 flex items-center gap-4 border border-line bg-panel p-6">
          <LoaderCircle className="animate-spin text-blue-400" size={24} />
          <div>
            <h2 className="text-sm font-semibold">Inspecting media</h2>
            <p className="mt-1 text-xs text-muted">The appropriate local engine is retrieving structured metadata…</p>
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
                {isGallery ? (
                  <span className="border border-line bg-elevated px-2 py-1">{analysis.itemCount ?? "Multiple"} original files</span>
                ) : (
                  <span className="border border-line bg-elevated px-2 py-1">{analysis.formats.length} streams detected</span>
                )}
                {analysis.totalSizeBytes ? (
                  <span className="border border-line bg-elevated px-2 py-1">Approximately {formatBytes(analysis.totalSizeBytes)}</span>
                ) : largestKnownSize(analysis.formats) ? (
                  <span className="border border-line bg-elevated px-2 py-1">Up to ~{formatBytes(largestKnownSize(analysis.formats))}</span>
                ) : null}
              </div>
            </div>
          </section>

          <section className="border border-line bg-panel p-5">
            <h2 className="mb-5 text-sm font-semibold">Download options</h2>
            <div className="grid gap-4">
              {isGallery ? (
                <div className="border border-line bg-elevated p-3 text-sm text-muted">
                  <p>gallery-dl preserves original album files. Video transcoding and container conversion do not apply.</p>
                  {!galleryItems ? (
                    <Button
                      variant="secondary"
                      className="mt-3 w-full"
                      disabled={galleryItemsLoading}
                      onClick={() => void loadGalleryItems(analysis.downloadUrl, true)}
                    >
                      {galleryItemsLoading ? <LoaderCircle className="animate-spin" size={16} /> : <Film size={16} />}
                      {galleryItemsLoading ? "Loading album files" : "Choose specific files"}
                    </Button>
                  ) : (
                    <div className="mt-3">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-xs font-medium text-ink">
                          {selectedGalleryItems.length} selected · {galleryItems.items.length} of {galleryItems.totalCount} loaded
                        </span>
                        <span className="text-xs">{selectedGallerySize > 0 ? `~${formatBytes(selectedGallerySize)}` : "Size unknown"}</span>
                      </div>
                      <Input
                        className="mt-3"
                        value={gallerySearch}
                        onChange={(event) => setGallerySearch(event.target.value)}
                        placeholder="Filter album files"
                      />
                      <div className="mt-2 flex gap-2">
                        <Button variant="secondary" className="h-8 px-2 text-xs" onClick={() => setSelectedGalleryIndices(galleryItems.items.map((item) => item.index))}>All</Button>
                        <Button variant="secondary" className="h-8 px-2 text-xs" onClick={() => setSelectedGalleryIndices(galleryItems.items.filter((item) => item.isVideo).map((item) => item.index))}>Videos</Button>
                        <Button variant="secondary" className="h-8 px-2 text-xs" onClick={() => setSelectedGalleryIndices([])}>Clear</Button>
                      </div>
                      {galleryItems.truncated ? (
                        <Button
                          variant="secondary"
                          className="mt-3 w-full"
                          disabled={galleryItemsLoading}
                          onClick={() => void loadGalleryItems(analysis.downloadUrl)}
                        >
                          {galleryItemsLoading ? <LoaderCircle className="animate-spin" size={16} /> : null}
                          {galleryItemsLoading ? "Loading more files" : "Load 12 more files"}
                        </Button>
                      ) : null}
                      <div className="mt-3 max-h-72 divide-y divide-line overflow-auto border border-line bg-panel">
                        {filteredGalleryItems.map((item) => (
                          <GalleryItemRow
                            key={item.index}
                            item={item}
                            selected={selectedGalleryIndices.includes(item.index)}
                            onChange={(selected) => setSelectedGalleryIndices((indices) => selected
                              ? [...indices, item.index]
                              : indices.filter((index) => index !== item.index))}
                          />
                        ))}
                        {filteredGalleryItems.length === 0 ? <p className="p-3 text-xs text-muted">No album files match this filter.</p> : null}
                      </div>
                    </div>
                  )}
                </div>
              ) : (
                <>
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
                </>
              )}

              <Field label="Destination" hint="The backend validates this directory before starting the engine.">
                <div className="flex gap-2">
                  <Input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} />
                  <Button variant="secondary" className="px-3" onClick={() => void chooseDestination()} aria-label="Choose destination folder">
                    <FolderOpen size={17} />
                  </Button>
                </div>
              </Field>

              <Field label="If the output already exists">
                <Select value={outputConflictPolicy} onChange={(event) => setOutputConflictPolicy(event.target.value as DownloadOptions["outputConflictPolicy"])}>
                  <option value="skip">Skip existing file</option>
                  <option value="overwrite">Overwrite existing file</option>
                </Select>
              </Field>

              {!isGallery ? <div className="border-t border-line pt-2">
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
              </div> : null}

              <Button className="mt-1 w-full" disabled={!destination || starting || galleryItemsLoading || Boolean(isGallery && galleryItems && selectedGalleryItems.length === 0)} onClick={() => void submitDownload()}>
                {starting ? <LoaderCircle className="animate-spin" size={17} /> : !isGallery && audioOnly ? <Music2 size={17} /> : <Download size={17} />}
                {starting ? "Adding to queue" : isGallery && galleryItems && (galleryItems.truncated || selectedGalleryItems.length !== galleryItems.items.length) ? `Download ${selectedGalleryItems.length} selected` : isGallery ? "Download album" : "Download"}
              </Button>
            </div>
          </section>
        </div>
      ) : null}

      {batchAnalysis && !analyzing ? (
        <section className="mt-5 border border-line bg-panel p-5">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="text-sm font-semibold">{batchAnalysis.collectionTitle || "Batch analysis"}</h2>
              <p className="mt-1 text-xs text-muted">Choose the successfully analyzed entries to add to the Rust queue.</p>
            </div>
            <span className="text-xs text-muted">{selectedBatchUrls.length} selected</span>
          </div>
          <div className="mt-4 max-h-80 divide-y divide-line overflow-auto border border-line">
            {batchAnalysis.items.map((item) => (
              <label key={item.inputUrl} className="flex items-center gap-3 p-3 text-sm">
                <input
                  type="checkbox"
                  disabled={!item.analysis}
                  checked={selectedBatchUrls.includes(item.inputUrl)}
                  onChange={(event) => setSelectedBatchUrls((selected) => event.target.checked ? [...selected, item.inputUrl] : selected.filter((urlValue) => urlValue !== item.inputUrl))}
                />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{item.analysis?.title || item.inputUrl}</div>
                  <div className={`mt-1 truncate text-xs ${item.error ? "text-red-400" : "text-muted"}`}>{item.error?.message || item.analysis?.siteName}</div>
                </div>
                {item.analysis?.durationSeconds != null ? <span className="text-xs text-muted">{formatDuration(item.analysis.durationSeconds)}</span> : null}
              </label>
            ))}
          </div>
          <div className="mt-4 grid gap-4 md:grid-cols-[1fr_220px_auto] md:items-end">
            <Field label="Destination">
              <div className="flex gap-2">
                <Input value={destination} onChange={(event) => setDestination(event.target.value)} spellCheck={false} />
                <Button variant="secondary" className="px-3" onClick={() => void chooseDestination()}><FolderOpen size={17} /></Button>
              </div>
            </Field>
            <Field label="Existing output">
              <Select value={outputConflictPolicy} onChange={(event) => setOutputConflictPolicy(event.target.value as DownloadOptions["outputConflictPolicy"])}>
                <option value="skip">Skip</option>
                <option value="overwrite">Overwrite</option>
              </Select>
            </Field>
            <Button disabled={starting || selectedBatchUrls.length === 0 || !destination} onClick={() => void submitBatch()}>
              {starting ? <LoaderCircle className="animate-spin" size={17} /> : <Download size={17} />}
              Queue {selectedBatchUrls.length}
            </Button>
          </div>
        </section>
      ) : null}
    </div>
  );
}

function parseInputUrls(value: string): string[] {
  return [...new Set(value.split(/\s+/).map((entry) => entry.trim()).filter(Boolean))];
}

function defaultBatchOptions(
  analysis: MediaAnalysis,
  destination: string,
  outputConflictPolicy: DownloadOptions["outputConflictPolicy"],
): DownloadOptions {
  if (analysis.engine === "galleryDl") {
    return {
      quality: analysis.qualityOptions[0]!,
      container: "mp4",
      videoCodecPreference: "any",
      audioQuality: "best",
      audioOnly: false,
      subtitles: false,
      subtitleLanguages: [],
      embedThumbnail: false,
      embedMetadata: false,
      chapters: false,
      sponsorblock: false,
      destination,
      filenameTemplate: "ignored",
      outputConflictPolicy,
      gallerySelection: null,
    };
  }
  const hasVideo = analysis.formats.some((format) => format.videoCodec);
  const quality = (hasVideo
    ? analysis.qualityOptions.find((option) => option.kind === "best")
    : analysis.qualityOptions.find((option) => option.kind === "audioOnly")) ?? analysis.qualityOptions[0]!;
  return {
    quality,
    container: hasVideo ? "mp4" : "m4a",
    videoCodecPreference: "any",
    audioQuality: "best",
    audioOnly: !hasVideo,
    subtitles: false,
    subtitleLanguages: [],
    embedThumbnail: false,
    embedMetadata: true,
    chapters: false,
    sponsorblock: false,
    destination,
    filenameTemplate: "%(title).180B [%(id)s].%(ext)s",
    outputConflictPolicy,
    gallerySelection: null,
  };
}

function GalleryItemRow({
  item,
  selected,
  onChange,
}: {
  item: GalleryItem;
  selected: boolean;
  onChange: (selected: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-center gap-3 p-2.5 hover:bg-elevated">
      <input type="checkbox" checked={selected} onChange={(event) => onChange(event.target.checked)} />
      <div className="h-11 w-16 shrink-0 overflow-hidden bg-black/25">
        {item.thumbnail ? <img className="h-full w-full object-cover" src={item.thumbnail} alt="" loading="lazy" /> : <div className="grid h-full place-items-center"><Film size={16} /></div>}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-xs font-medium text-ink">{item.filename}{item.extension ? `.${item.extension}` : ""}</div>
        <div className="mt-1 flex gap-2 text-[11px] text-muted">
          <span>#{item.index}</span>
          <span>{item.isVideo ? "Video" : "File"}</span>
          {item.sizeBytes != null ? <span>{formatBytes(item.sizeBytes)}</span> : null}
        </div>
      </div>
    </label>
  );
}

function largestKnownSize(formats: { approximateFileSize?: number | null }[]): number | null {
  const sizes = formats.flatMap((format) => format.approximateFileSize ?? []);
  return sizes.length ? Math.max(...sizes) : null;
}
