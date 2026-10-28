# M59 — Large amend review

The amend composer rejected a complete staged selection whenever the combined patch exceeded 512 KiB. The same review also disabled the existing per-file preview path for amendments. This made a 107-file amendment impossible even though its complete staged manifest and exact replacement tree were available.

Amend now uses the same bounded per-file code loading as ordinary commit review. The existing Staged group retains the complete selection, and selecting a file loads the HEAD-to-index delta into the existing diff dock. No new surface, IPC operation or higher memory budget is needed. An aggregate truncated patch remains marked as truncated; an individual oversized patch also keeps its partial-preview notice.

Confirmation still revalidates the reviewed state and exact replacement tree. Author/date, parents, message-only amendment, referenced-history acknowledgment, original-commit recovery and preservation of unstaged files remain unchanged. No developer repository is amended during verification.
