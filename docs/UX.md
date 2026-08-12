# Desktop UX Principles

## Visual system

The desktop shell uses a graphite and verdigris palette, Cantarell-first Linux typography, compact squared controls, and restrained tinted depth. Accent color identifies the current route, primary action, live progress, and focus state; it is not decorative. Panels use hierarchy and spacing before borders, while tabular transfer data uses a monospace stack. Motion is limited to short hover, press, progress, and disclosure transitions.

The sidebar separates the primary New download action from workspace navigation and system status. Page headers use a consistent eyebrow, title, description, and action rhythm. Empty and no-results states explain what the application is waiting for rather than presenting a blank table.

Optional clipboard detection only runs while New Download is visible and offers a copied URL for confirmation. It never starts analysis or a download. A matched site rule is announced before applying its preset, and selecting another preset clears the automatic match. Filename previews are advisory; Rust performs authoritative validation.

Duplicate protection uses a durable archive separate from History. When the current URL and destination match an archived completion, the options panel explains the match and offers an explicit Download again action.

Nice Downloader presents download intent, not engine flags. The normal path is `Paste URL → Analyze → Confirm recommendation → Download`. Quality, destination, and the recommended strategy remain visible; codecs, subtitles, naming, and overwrite behavior use progressive disclosure.

Navigation must not discard work. Page components remain mounted so analyzed metadata, gallery selections, filters, and option edits survive a trip to Queue or Settings. The selected page and sidebar state persist locally. `Ctrl+N` opens New Download and `Ctrl+,` opens Settings.

Queue authority remains in Rust. The frontend displays queue position, state, progress, speed, ETA, operation, attempt number, and sanitized engine logs. Queue pause means “do not start additional jobs”; per-job pause controls a running or waiting job. Cancelling preserves resumable partial data by default. Drag reordering supplements, but does not replace, keyboard-accessible arrow controls.

Errors lead with a user explanation and route to the relevant recovery surface. Authentication and engine failures link to Settings, duplicate downloads link to History, and retry/rate-limit failures link to Queue. Technical diagnostics remain expandable.

History is a reuse surface, not only a log. “Again” loads the persisted typed request in Rust and queues a new overwrite-authorized attempt; it does not reconstruct engine arguments in React. Open, reveal, copy URL, search, and record-only deletion remain separate actions.

Color is semantic: blue is active, amber is waiting or retrying, violet is verification, green is completed/verified, and red is failure or destructive action. Animation is limited to active progress and loading indicators. Transfer numbers use tabular figures.

Collection workflows support search, video-only selection, inversion, numeric ranges, incremental loading, selected count, and estimated size without eagerly resolving an entire remote album.
