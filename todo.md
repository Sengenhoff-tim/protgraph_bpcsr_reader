# TODO

Follow-ups to the arena graph layout (`src/process_graphs/graph/protein_graph.rs`).

## Traversal

- [ ] **Outer loop per query interval.** Run the traversal once per query interval, re-reading
  the converted `.pgarena` file each pass, instead of checking all intervals at once.
  With one interval per pass, `has_overlapping_interval` reduces to a single overlap check per node.

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
  - Same sequence can be emitted with and without mod (handled by `deduplicate_output`,
    unless the mod/weight should become part of the record).

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

## Conversion stage

- [ ] Parallelize conversion (reader thread → converter workers → ordered writer); currently serial.
- [ ] Optional full arena validation on load (block headers and edge targets point at block starts),
  so a corrupt `.pgarena` errors instead of panicking.
- [ ] Support gzipped `.bpcsr` input.
