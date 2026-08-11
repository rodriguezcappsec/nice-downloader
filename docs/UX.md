# Desktop UX Principles

Optional clipboard detection only runs while New Download is visible and offers a copied URL for confirmation. It never starts analysis or a download. A matched site rule is announced before applying its preset, and selecting another preset clears the automatic match. Filename previews are advisory; Rust performs authoritative validation.

Duplicate protection uses a durable archive separate from History. When the current URL and destination match an archived completion, the options panel explains the match and offers an explicit Download again action.

Nice Downloader presents download intent, not engine flags. The normal path is `Paste URL → Analyze → Confirm recommendation → Download`. Quality, destination, and the recommended strategy remain visible; codecs, subtitles, naming, and overwrite behavior use progressive disclosure.

Navigation must not discard work. Page components remain mounted so analyzed metadata, gallery selections, filters, and option edits survive a trip to Queue or Settings. The selected page and sidebar state persist locally. `Ctrl+N` opens New Download and `Ctrl+,` opens Settings.

Queue authority remains in Rust. The frontend displays queue position, state, progress, speed, ETA, operation, attempt number, and sanitized engine logs. Queue pause means “do not start additional jobs”; per-job pause controls a running or waiting job. Cancelling preserves resumable partial data by default. Drag reordering supplements, but does not replace, keyboard-accessible arrow controls.

Errors lead with a user explanation and route to the relevant recovery surface. Authentication and engine failures link to Settings, duplicate downloads link to History, and retry/rate-limit failures link to Queue. Technical diagnostics remain expandable.

History is a reuse surface, not only a log. “Again” loads the persisted typed request in Rust and queues a new overwrite-authorized attempt; it does not reconstruct engine arguments in React. Open, reveal, copy URL, search, and record-only deletion remain separate actions.

Color is semantic: blue is active, amber is waiting or retrying, violet is verification, green is completed/verified, and red is failure or destructive action. Animation is limited to active progress and loading indicators. Transfer numbers use tabular figures.

Collection workflows support search, video-only selection, inversion, numeric ranges, incremental loading, selected count, and estimated size without eagerly resolving an entire remote album.
