# Library Accessibility QA

Use this checklist for keyboard-only and Orca verification of the Library. Test at 100% interface scale first, then repeat the scaling and contrast checks below.

## Setup

Install and start Orca if it is not already available:

```bash
# Arch Linux
sudo pacman -S orca

# Debian or Ubuntu
sudo apt install orca

orca --replace
```

Launch Nice Downloader with `npm run tauri:dev`. Prepare a source containing more than 100 supported files, at least two exact duplicate files, one playable video, and multiple supported images. Do not use sensitive filenames when recording QA output.

## Keyboard path

1. Press `Tab` from application start. Confirm the skip link can move focus directly to main content.
2. Reach Library search, filter chips, folder scope, view controls, media cards, pagination, and Manage using only `Tab`, `Shift+Tab`, `Enter`, Space, and arrow keys where native controls support them.
3. Open media details from a card. Confirm the close button receives focus, `Tab` and `Shift+Tab` stay in the panel, and left/right navigation does not lose the original return target. Close the panel and confirm focus returns to the invoking card.
4. Open Watch, start playback, search the right rail, change result pages, and switch videos. Activate Library without a pointer; confirm playback continues in the bottom-right mini player without a loading restart. Tab to its system-player, restore, and close controls. Restore the full Watch workspace, then navigate elsewhere and confirm the mini player remains operable without intercepting that workspace's playback shortcuts.
5. Open an image from details. Confirm full-resolution loading is announced, previous/next follows the visible Library order, zoom/fit/rotate/slideshow controls are keyboard reachable, and focused controls do not double-trigger viewer shortcuts. Open details and confirm image shortcuts pause until it closes. With video minimized, view an image and restore Watch; only one full media workspace should remain. Close the viewer and confirm focus returns to the invoking card or Library search if that card disappeared.
6. Open Bulk Move. Confirm `Tab` and `Shift+Tab` stay inside, Escape closes it, and focus returns to Move. After a successful move removes the bulk bar, focus should fall back to Library search.
7. Open Add folder. Confirm Browse receives focus and closing or completing the form restores focus to Add folder.

If filtering, moving, or removing media deletes the original focus target, focus must fall back to Library search and then main content. Escape from an active title, tag, or filename field must not discard the form or close media details.

## Expected announcements

- Changing Library search, filters, or pages announces the visible range, total matches, and page count once.
- A scan announces its start, each 100-file discovery boundary, and completion, cancellation, or failure. Individual indexed files are not announced.
- Duplicate analysis announces candidate discovery, 20% boundaries, and its final group/verified/skipped counts. It must not speak every filename.
- Watch search announces loading followed by its result count and page.
- Failures use an assertive announcement while routine progress stays polite.

## Display and motion

- At 90%, 100%, 110%, and 125%, focused controls remain visible without clipped labels or unreachable actions.
- High contrast keeps text, focus rings, selected filters, warnings, and destructive controls distinguishable.
- Reduced motion disables nonessential scaling and animated transitions while preserving progress meaning.

Record the OS, WebKitGTK, Orca, scale, theme, source size, and any missed, duplicated, or overly frequent announcement with the test result.

## Current verification status

The TypeScript production build verifies the implemented focus targets and dialog handlers compile. Manual keyboard and Orca verification remains required on a Linux desktop with Orca installed; do not treat a build-only result as assistive-technology approval.
