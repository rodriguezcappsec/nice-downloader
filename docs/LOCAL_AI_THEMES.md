# Local AI Theme Discovery

Theme discovery is an optional, local-only Library index. It classifies what videos are about so media from different creators can share a filter such as `ASMR`, `Ear massage`, or `Mic brushing`.

## Runtime setup

Nice Downloader connects only to Ollama's fixed loopback endpoint at `127.0.0.1:11434`. It does not accept a provider URL from React and does not upload frames, transcripts, titles, or model responses.

1. Install and start Ollama.
2. Download a vision-capable model, for example `ollama pull qwen2.5vl:7b`.
3. Open **Library > Discover video themes**.
4. Select the installed model and analysis depth, explicitly enable the feature, and save.
5. Start the stale-video analysis job.

The model is not bundled because Qwen2.5-VL 7B weights are several gigabytes and hardware requirements differ. Another installed Ollama vision model can be selected, but Nice Downloader cannot verify that an arbitrary model actually accepts images.

## Analysis modes

- **Deep** is the default. FFmpeg samples six chronological frames into one bounded JPEG contact sheet. The model receives that image plus bounded title, tags, description, and an excerpt from any already-indexed WebVTT transcript.
- **Fast** sends the bounded textual context without frames. It is useful for large metadata-rich libraries but must not claim visual evidence.

The prompt treats all media text and visible text as untrusted content. Model output must parse as bounded JSON, themes below 45 percent confidence are discarded, and evidence is retained so users can see why a theme was assigned. Confidence remains advisory.

## Index and filtering behavior

Theme assignments are rebuildable derived state keyed by catalog UUID and the observed source size/modified-time fingerprint. Changing a source file, selected model, or analysis depth makes the relevant item stale. Stale-fingerprint assignments are excluded from Library facets and filters until analysis succeeds again.

The Library exposes one selected theme at a time. Theme keys are normalized creator-independent labels, so videos from unrelated creators can both match `Ear massage`. The model is instructed to emit broad and specific reusable concepts instead of copying titles or creator names.

Theme analysis is sequential and cancellable. A sampling or malformed-response failure skips that item; loss of the local model stops the job instead of producing thousands of failures. No theme data is included in Library backup because it can be rebuilt from the user's selected local model.
