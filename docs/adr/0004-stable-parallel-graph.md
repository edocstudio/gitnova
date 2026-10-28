# ADR 0004: Stable parallel graph lanes

Status: accepted, 2026-10-25.

The initial graph visited one line of development to completion, compacted active lane positions, and used column position as color identity. On repositories with many branches this obscured parallel work and changed colors when unrelated lanes finished.

Use Git's `--date-order` traversal. It interleaves parallel histories by commit date while keeping every child ahead of its parents, including timestamp skew. `--topo-order` intentionally avoids intermixing histories; that property is less useful for this view. See the [official Git ordering documentation](https://git-scm.com/docs/git-log#_commit_ordering).

Reserve column zero for the checked-out revision, without drawing an incoming line until a descendant actually reaches it. Cursor version 2 stores sparse lane slots, immutable color identities, and whether a pending revision has a real incoming edge. Keep occupied columns in place; reuse vacant slots and trim only empty trailing columns. First-parent continuations retain their color; new side histories receive their own color. Carry this state through pagination. Reject old cursor versions and changed repository fingerprints.

The frontend renders explicit incoming strokes, rounded outgoing edges, ring-shaped merge nodes and a separate HEAD ring. A dedicated Branch / Tag column moves existing reference badges within the graph; it adds no navigation surface. Co-located local and remote names share a badge only at the same commit. Tags remain distinct. Overflow preserves full reference names in the tooltip and existing inspector. Compact 36px rows retain keyboard selection, virtualization and per-repository scroll restoration.

Colors identify graph lines within a loaded session, not permanent branch identities: branches can share history and refs can move. Refresh may allocate colors differently. The small palette cycles for large numbers of concurrent histories. Sparse layouts may be wider than compacted layouts; horizontal scrolling preserves topology. Offset pagination still repeats Git traversal and is not a persistent revision walker.

Tests cover every split of an octopus fixture, criss-cross histories, disappearing lanes, disconnected tips and a reserved HEAD. A real 121-commit fixture verifies interleaved tips, child-before-parent ordering and lane/color continuity across the 100-row page boundary. Browser tests read actual Rust/Git results and verify reference grouping, compact rows and parallel lanes.
