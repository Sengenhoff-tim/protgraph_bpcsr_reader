# TODO

Follow-ups to the arena graph layout (`src/process_graphs/graph/protein_graph.rs`).

## Traversal

- [x] **Outer loop per query interval** with in-memory dedup and spilling (`src/pipeline.rs`).
- [ ] `TraversalContext.intervals` always holds one interval now: make it a single `Interval`
  so `has_overlapping_interval` becomes a plain overlap check.
- [ ] Many narrow intervals mean many full passes over the graph file. Consider merging
  nearby intervals before the loop (CLI option), or memory-mapping the `.pgarena` file
  (needs 8-byte alignment of the arena in the file format).

- [ ] **Derive weights from sequence.** Drop `mono_weight` from the node header and compute it
  from the residue codes (32-entry mass table indexed by code).
  - Check first: computed weight == stored `mono_weight` for every node on real data
    (rounding per residue vs. total, H₂O on start/end node, masses of B/Z/X/U/O/J,
    fixed mods already included in the stored weight?).

- [ ] **Multiple weights per node (variable mods).** In the recursion, loop over the possible
  node weights per edge:
  ```text
  for edge in node.edges():
      for (w, mods) in target.weights(state.mods, ctx):   // by mod count, not position
          if prune check passes with tv + w: recurse
  ```
  - `weights()` as iterator / small array, no allocation; nodes without modifiable residues
    yield exactly one weight.
  - add `mods` to `PathState`, `max_mods` to `TraversalContext`.
  - protgraph computes pdbs with the same mods, so no interval widening is needed —
    **but our mod config must match protgraph's exactly** (more mods here = wrong pruning).
    Optional debug check: warn when a completed path's exact weight is outside the root's pdbs.
  - **Breaks the per-interval dedup assumption**: the same sequence with and without a mod has
    two weights and can occur in two intervals, so it would be written twice.
    Plan — **owner interval**: a sequence's possible weights (`base + k·δ`, or combinations of
    counts for several mod types) depend only on the sequence and the mod config. Each sequence
    is owned by the interval matched by its variant with the fewest mods. A worker emits a
    completed path only if the current interval is the owner (owner = first interval hit by
    `base + k'·δ` for `k' = 0, 1, …`, binary search over the sorted intervals); otherwise it
    drops it, because the owner pass emits it. All proteins compute the same owner, so all
    accessions meet in one interval's table and per-interval dedup stays exact. Cheap: only
    completed paths with modifiable residues, no extra memory.
    - **Context-dependent mods** (protein N-term acetylation, initiator Met, C-term mods) break
      this: the owner variant may exist in one protein but not another, and that protein's
      accession would be lost. Route such records past the per-interval table into a
      **global spill set** (`tmp/global/`, hash-sharded across all intervals), merged by the
      existing final shard dedup.
    - Assumes the downstream search applies mods itself (output stays unmodified sequences).
      Putting the mod state into the dedup key instead would only make sense if modified
      peptides should be separate output entries.
    - Open: which mods are used (residue-only → owner rule suffices, no global spill set)?

- [ ] **Binned pdbs (u16 / u32, user-settable).** Store pdb intervals as bins in the arena:
  - bin width `2^k` units (1e-9 Da), `k` derived from user max and bin type → quantize by shift.
  - node intervals rounded outwards (lo floor, hi ceil), shifted query bounds rounded outwards
    per step → binned check is a superset of the exact one.
  - drop intervals with `lo > max` at conversion time, clamp `hi` (also removes the
    `i64::MAX` sentinel).
  - **exact final check** at the terminal node: accumulated exact weight vs query interval.
  - generic `B: Bin` so u16/u32 are monomorphized.
  - conversion then depends on max weight → store `k` / bin type in the `.pgarena` header and
    reject mismatching runs.

## Dedup

- [ ] Measure the hit rate per interval (logged per writer) on real data and tune the default
  `--bucket_size` and the 80/20 split between slots and heap in `TableConfig::from_budget`.
- [x] Each `sequence → accession` mapping is stored once (table and shard dedup).
- Backpressure from writers to workers comes from the bounded batch buffer pool: a worker
  needs an empty buffer to send a full one, so it blocks while the writers are behind.
  The writer channels themselves are unbounded; the pool size limits what is in flight.

## Conversion stage

- [ ] Parallelize conversion (reader thread → converter workers → ordered writer); currently serial.
- [ ] Optional full arena validation on load (block headers and edge targets point at block starts),
  so a corrupt `.pgarena` errors instead of panicking.
- [ ] Support gzipped `.bpcsr` input.
