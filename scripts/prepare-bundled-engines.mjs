import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { arch, platform } from "node:process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = path.join(repositoryRoot, "src-tauri", "resources", "engines");
const cacheDirectory = path.join(repositoryRoot, ".cache", "bundled-engines");
const lock = JSON.parse(await readFile(path.join(repositoryRoot, "engines.lock.json"), "utf8"));
const platformKey = "linuxX64";

if (platform !== "linux" || arch !== "x64") {
  throw new Error(`Bundled engines support Linux x64 only; received ${platform}/${arch}.`);
}

await mkdir(outputDirectory, { recursive: true });
await mkdir(cacheDirectory, { recursive: true });

async function run(command, args, options = {}) {
  const { quiet = false, ...spawnOptions } = options;
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: quiet ? "ignore" : "inherit", ...spawnOptions });
    child.once("error", reject);
    child.once("exit", (code) => code === 0 ? resolve() : reject(new Error(`${command} exited with ${code}`)));
  });
}

async function downloadVerified(name, asset) {
  const destination = path.join(cacheDirectory, name);
  let bytes;
  try {
    bytes = await readFile(destination);
  } catch {
    const response = await fetch(asset.url, { redirect: "follow" });
    if (!response.ok) throw new Error(`Could not download ${asset.url}: HTTP ${response.status}`);
    bytes = Buffer.from(await response.arrayBuffer());
    await writeFile(destination, bytes);
  }
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== asset.sha256) {
    await rm(destination, { force: true });
    throw new Error(`${name} checksum mismatch: expected ${asset.sha256}, received ${actual}`);
  }
  return destination;
}

const ytDlpSource = await downloadVerified(`yt-dlp-${platformKey}`, lock.ytDlp[platformKey]);
const ytDlpOutput = path.join(outputDirectory, "yt-dlp");
await copyFile(ytDlpSource, ytDlpOutput);

for (const engine of ["ffmpeg", "ffprobe"]) {
  const archive = await downloadVerified(`${engine}-${platformKey}.zip`, lock.ffmpeg[platformKey][engine]);
  await run("unzip", ["-o", "-j", archive, engine, "-d", outputDirectory]);
}

const wheel = await downloadVerified(
  `gallery_dl-${lock.galleryDl.version}-py3-none-any.whl`,
  lock.galleryDl.wheel,
);
const buildDirectory = path.join(cacheDirectory, `gallery-dl-build-${platformKey}`);
const venvDirectory = path.join(buildDirectory, "venv");
await rm(buildDirectory, { recursive: true, force: true });
await mkdir(buildDirectory, { recursive: true });
await run("python3", ["-m", "venv", venvDirectory]);
const venvPython = path.join(venvDirectory, "bin", "python");
await run(venvPython, ["-m", "pip", "install", "--disable-pip-version-check", `pyinstaller==${lock.galleryDl.pyinstallerVersion}`, wheel]);
const entrypoint = path.join(buildDirectory, "gallery-dl-entry.py");
await writeFile(entrypoint, "import gallery_dl\nraise SystemExit(gallery_dl.main())\n");
await run(venvPython, [
  "-m", "PyInstaller", "--clean", "--noconfirm", "--onefile",
  "--collect-all", "gallery_dl",
  "--name", "gallery-dl", "--distpath", outputDirectory,
  "--workpath", path.join(buildDirectory, "work"),
  "--specpath", buildDirectory, entrypoint,
]);

await Promise.all([ytDlpOutput, "ffmpeg", "ffprobe", "gallery-dl"].map((name) =>
  chmod(path.isAbsolute(name) ? name : path.join(outputDirectory, name), 0o755)));

for (const [engine, versionArgument] of [["yt-dlp", "--version"], ["gallery-dl", "--version"], ["ffmpeg", "-version"], ["ffprobe", "-version"]]) {
  await run(path.join(outputDirectory, engine), [versionArgument]);
}
await run(path.join(outputDirectory, "gallery-dl"), ["--list-extractors"], { quiet: true });

console.log(`Prepared verified bundled engines in ${outputDirectory}`);
