# Desktop design review

Reviewed and corrected on 2026-11-15. Scope: all four egui pages, navigation,
light/dark themes, populated and empty states, import forms, deletion and settings.
The web-interface review checklist was applied only where it makes sense for a
native application: focus, input labels, hit targets, feedback and overflow.

| Finding | Correction |
| --- | --- |
| Navigation had an outer selected frame and an inner selectable label with separate hover feedback | One native egui button per row, with shared icon/text/background hit area |
| Sidebar content reserved about 215.75 px while the panel clipped painting at 180 px | Fixed 216 px panel; bounded full-width buttons; no separator line or unpainted strip |
| Icon clicks did not select a page | Entire row, including the icon and trailing space, selects the page |
| Screen readers encountered private icon glyphs instead of clean navigation labels | Explicit labels and selected-state accessibility metadata |
| Horizontal toolbars and nested server headings could exceed narrow content bounds | Wrapping toolbars, a bounded search field and directly wrapping server labels |
| The primary button's fixed fill suppressed hover feedback | Explicit normal, hovered and pressed colors through egui's widget states |
| Small action buttons had inconsistent hit target heights | Standard action buttons use the common interaction height |
| Start was available without a valid selected server | Start is disabled while Choose server remains accessible |
| An import was an ordinary window, and failed submission closed it and erased input | Modal form, inline error, retained input until success/cancel, bounded scrolling and explicit cancel |
| Hidden pasted YAML could not be inspected intentionally | Explicit Show contents control, off by default; fields have accessible labels |
| Settings checkboxes edited the saved snapshot and could be reset by asynchronous updates | Separate settings draft preserved across updates, saved only by Save settings |
| Removal happened immediately | Confirmation modal with a cancel action before deletion is queued |

## Verification

Two regression tests failed before the navigation fix: the painted/reserved-width
check measured 215.75 versus 180 px, and clicking the icon did not select its page.
Both pass after the fix. Other UI tests cover:

- All four pages in both themes at 720x560 and 980x780, with long server/source
  names and bounded card geometry.
- Settings drafts surviving status updates without modifying saved settings.
- Failed imports keeping their input and showing the error inside the form.
- Canceling removal without sending a delete action.

`cargo test --locked`: 19 passed, 2 real-core tests intentionally ignored.
`cargo fmt --all -- --check` and Clippy with warnings denied: passed.
The two core/network integration tests were not rerun for this UI-only change.
No engine or routing behavior was changed; Settings gained equality comparison.

The rebuilt macOS .app was launched and checked directly. Verified icon clicks,
selected-state accessibility values, keyboard Tab/Return activation, empty states,
both palettes, import error retention, and Escape dismissal. Current screenshots
are authentic first-launch views of this build, with no user credentials.

A complete VoiceOver session and Linux desktop rendering were not checked.
