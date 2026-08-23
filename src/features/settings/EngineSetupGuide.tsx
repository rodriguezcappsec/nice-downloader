import { Check, Copy, PackageOpen, Terminal } from "lucide-react";
import { useState } from "react";
import { Button } from "../../components/ui";

type DistributionId = "arch" | "debian" | "fedora" | "opensuse";
type CopyTarget = "required" | "optional";

interface DistributionGuide {
  id: DistributionId;
  label: string;
  packageManager: string;
  requiredCommand: string;
  optionalCommand: string;
  note?: string;
}

const distributionGuides: DistributionGuide[] = [
  {
    id: "arch",
    label: "Arch Linux",
    packageManager: "pacman",
    requiredCommand: "sudo pacman -S --needed yt-dlp ffmpeg",
    optionalCommand: "sudo pacman -S --needed gallery-dl aria2 curl",
  },
  {
    id: "debian",
    label: "Debian / Ubuntu",
    packageManager: "APT",
    requiredCommand: "sudo apt update\nsudo apt install yt-dlp ffmpeg",
    optionalCommand: "sudo apt install gallery-dl aria2 curl",
  },
  {
    id: "fedora",
    label: "Fedora",
    packageManager: "DNF",
    requiredCommand: "sudo dnf install yt-dlp ffmpeg-free",
    optionalCommand: "sudo dnf install gallery-dl aria2 curl",
    note: "Fedora's ffmpeg-free package has limited codec coverage. Full FFmpeg support may require the RPM Fusion repositories.",
  },
  {
    id: "opensuse",
    label: "openSUSE",
    packageManager: "Zypper",
    requiredCommand: "sudo zypper install yt-dlp ffmpeg",
    optionalCommand: "sudo zypper install gallery-dl aria2 curl",
    note: "Some codecs may require multimedia packages from the Packman repository.",
  },
];

export function EngineSetupGuide() {
  const [selectedDistribution, setSelectedDistribution] = useState<DistributionId>("debian");
  const [copied, setCopied] = useState<CopyTarget | null>(null);
  const [copyFailed, setCopyFailed] = useState(false);
  const guide = distributionGuides.find((item) => item.id === selectedDistribution) ?? distributionGuides[0];

  async function copyCommand(target: CopyTarget, command: string) {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(target);
      setCopyFailed(false);
    } catch {
      setCopied(null);
      setCopyFailed(true);
    }
  }

  return (
    <section className="mb-8" aria-labelledby="system-engine-setup-title">
      <div className="mb-3 flex items-start gap-3">
        <div className="grid size-9 shrink-0 place-items-center rounded-[8px] border border-accent/20 bg-accent/[0.07] text-accent">
          <PackageOpen size={17} />
        </div>
        <div>
          <h2 id="system-engine-setup-title" className="text-sm font-semibold">System engine setup</h2>
          <p className="mt-1 max-w-3xl text-xs leading-5 text-muted">
            Choose your distribution and copy the commands into a terminal. Nice Downloader never runs package-manager commands itself. FFprobe is installed with the FFmpeg package.
          </p>
        </div>
      </div>

      <div className="panel-shadow overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
        <div className="flex flex-wrap gap-1 border-b border-line/60 bg-elevated/25 p-2" role="tablist" aria-label="Linux distribution">
          {distributionGuides.map((item) => (
            <button
              key={item.id}
              id={`engine-guide-tab-${item.id}`}
              type="button"
              role="tab"
              aria-selected={selectedDistribution === item.id}
              aria-controls="engine-guide-panel"
              className={`rounded-[7px] px-3 py-2 text-xs font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${selectedDistribution === item.id ? "bg-accent/12 text-accent" : "text-muted hover:bg-elevated hover:text-ink"}`}
              onClick={() => {
                setSelectedDistribution(item.id);
                setCopied(null);
                setCopyFailed(false);
              }}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div
          id="engine-guide-panel"
          role="tabpanel"
          aria-labelledby={`engine-guide-tab-${guide.id}`}
          className="p-4"
        >
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <div>
              <h3 className="text-sm font-semibold text-ink">{guide.label}</h3>
              <p className="mt-1 text-xs text-muted">Commands use {guide.packageManager} and may request your administrator password in the terminal.</p>
            </div>
            <span className="rounded-[5px] border border-line/70 bg-canvas/50 px-2 py-1 font-mono text-[11px] text-muted">Linux x86-64</span>
          </div>

          <div className="grid gap-3 lg:grid-cols-2">
            <CommandCard
              title="Required engines"
              description="yt-dlp handles URL analysis and downloads. FFmpeg handles merging, conversion, thumbnails, and compatibility playback."
              command={guide.requiredCommand}
              copied={copied === "required"}
              onCopy={() => void copyCommand("required", guide.requiredCommand)}
            />
            <CommandCard
              title="Recommended helpers"
              description="gallery-dl adds gallery sources, aria2 improves resumable direct downloads, and curl remains the fallback."
              command={guide.optionalCommand}
              copied={copied === "optional"}
              onCopy={() => void copyCommand("optional", guide.optionalCommand)}
            />
          </div>

          {guide.note ? <p className="mt-4 border-l-2 border-amber-400/60 bg-amber-400/[0.05] px-3 py-2 text-xs leading-5 text-amber-200">{guide.note}</p> : null}
          <p className="mt-4 text-xs leading-5 text-muted">
            Prefer app-managed tools? yt-dlp and gallery-dl can also be installed through <strong className="font-semibold text-ink">Managed engine versions</strong> below. After installing anything, select <strong className="font-semibold text-ink">Detect again</strong>.
          </p>
          <p className={`mt-2 min-h-5 text-xs ${copyFailed ? "text-red-300" : "text-emerald-400"}`} role="status" aria-live="polite">
            {copyFailed ? "Clipboard access failed. Select the command text and copy it manually." : copied ? "Command copied to the clipboard." : ""}
          </p>
        </div>
      </div>
    </section>
  );
}

function CommandCard({ title, description, command, copied, onCopy }: { title: string; description: string; command: string; copied: boolean; onCopy: () => void }) {
  return (
    <article className="rounded-[9px] border border-line/60 bg-canvas/45 p-4">
      <div className="flex items-center gap-2">
        <Terminal size={15} className="text-accent" />
        <h4 className="text-sm font-semibold text-ink">{title}</h4>
      </div>
      <p className="mt-2 min-h-10 text-xs leading-5 text-muted">{description}</p>
      <pre className="mt-3 min-h-[72px] overflow-x-auto whitespace-pre-wrap break-words rounded-[7px] border border-line/70 bg-canvas/80 p-3 font-mono text-xs leading-5 text-ink"><code>{command}</code></pre>
      <Button variant="secondary" className="mt-3 h-8 px-2.5 text-xs" onClick={onCopy} aria-label={`Copy ${title.toLowerCase()} command`}>
        {copied ? <Check size={14} /> : <Copy size={14} />} {copied ? "Copied" : "Copy command"}
      </Button>
    </article>
  );
}
