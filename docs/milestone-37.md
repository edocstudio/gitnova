# M37 — Version tags and explicit single-tag Push

Create tag replaces the lightweight-only graph menu entry. Select the exact
commit, enter its version/name, choose Annotated (default) or Lightweight and
review before creation. Annotation defaults to the tag name if omitted. Tags are
explicitly unsigned in this slice; signing is not silently inferred from config.

Creation returns to the same selected graph commit with Push tag immediately
available. Existing tag badges also offer Push tag in their context menu. The
review names one configured remote, live destination, tag object, peeled commit
and annotation. Same-object remote tags are idempotent; a different object blocks
publication. The main Push action remains branch-only.

Native previews bind the checkout/index/config, exact local ref and live remote
tag. Apply revalidates, uses a single immutable object-to-tag refspec without
force/mirror/follow-tags or submodule publication, and verifies both destination
and local ref afterward. Creation checks the resulting annotation or lightweight
object. Git's verbatim annotation bytes are preserved, including final-newline
presence. Hooks/signing at publication remain explicit. Source files and staging
stay intact; dirty working files are allowed. Local and remote outcomes have
separate receipts; unknown network outcomes require inspection, not auto-retry.

Validation: temporary real Git source and bare remote; annotated non-HEAD target,
Unicode annotation, dirty/staged preservation, exact object publication, no other
refs pushed, same-object verification, duplicate name, moved local tag and
conflicting remote object. Browser coverage exercises annotated and lightweight
flows, immediate Push, graph context Push and existing checkout/branch Push.
Native Tauri coverage includes local creation and origin rejection. EN/TH labels
and the native payload contracts are checked separately.

Bounded annotations: 64 KiB create / 128 KiB existing tag inspection. Existing
single-destination push restrictions apply. No releases, tag deletion on a remote,
force replacement, signed local tags or bulk tag publication are added.

[Real-Git tag publication review](screenshots/version-tag-publish.png).
