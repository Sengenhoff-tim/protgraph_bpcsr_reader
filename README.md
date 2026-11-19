rust implementation with dedublication for https://github.com/mpc-bioinformatics/ProGFASTAGen/tree/main/bin/ProtGraphTraverseIntSourceVarLimitter

## Usage

This tool processes protein graphs against mass range queries to identify matching peptide sequences. It performs graph traversal, variant enumeration, and deduplication with configurable memory and processing constraints.

### Required Arguments

| Argument | Short | Long | Description |
|----------|-------|------|-------------|
| Graph file | `-g` | `--graphs` | Path to ProtGraph `.bpcsr` protein graph file |
| Query file | `-q` | `--queries` | CSV file with mass ranges (format: `lower,upper` in Da, one per line) |
| Output directory | `-o` | `--outdir` | Directory for results |

### Functional Parameters

| Argument | Short | Long | Default | Description |
|----------|-------|------|---------|-------------|
| Max variants | `-v` | `--max_vars` | 3 | Maximum sequence variants per peptide, at most 6 |
| Max missed cleavages | `-c` | `--max_missed_cleavages` | 3 | Maximum cleavage misses. For full digestion, this describes maximum peptide length |
| Global lower bound | `-l` | `--lower_bound` | 100 | Minimum peptide mass (Da), clamps as sanity check |
| Global upper bound | `-u` | `--upper_bound` | 10000 | Maximum peptide mass (Da), clamps as sanity check |
| Zip output | `-z` | `--zip` | false | Compress output files |

### Performance & Memory

| Argument | Default | Description |
|----------|---------|-------------|
| `--avail_processors` | 1 | CPU cores for parallel processing |
| `--avail_memory` | Required | Available RAM in GB; 80% is used for the in-memory dedup tables |
| `--bucket_size` | 8 | Entries per bucket of the in-memory dedup tables |
| `--max_file_handles` | 80% of RLIMIT_NOFILE, clamped 64–8192 (Windows: 2048) | Spill files open at once during traversal, over all writers |

Queue depths and buffer sizes are derived from `--avail_processors` and `--avail_memory`.

### How it works

1. **Conversion**: the `.bpcsr` input is converted once into `<outdir>/graphs.pgarena`
   (one memory block per graph node, global accession table). Passing a `.pgarena` file as
   `-g` skips this step.
2. **Traversal**: all graphs are traversed once against all query intervals. Results are
   deduplicated in memory by sequence; when memory runs out, entries are spilled to
   `<outdir>/tmp`. Complete results are written to the output right away.
3. **Spilled entries** are deduplicated from their files at the end and appended.

`<outdir>/tmp` is cleared at the start of a run and removed at the end.

### Output

| File | Content |
|------|---------|
| `peptides.fasta` | `>pg|{start}:{count - 1}` and the sequence, 60 residues per line |
| `metadata.bin` | first byte: id width `w` (1–4); then the accession ids of all peptides in fasta order, `w` bytes little endian each. Peptide ids `start .. start + count` belong to the header |
| `accessions.csv` | `id,accession` |

With `-z`, all three are gzip compressed. Each accession is listed at most once per peptide.

### Minimal Example
```
bpcsr_to_fasta \
  -g proteins.bpcsr \
  -q ranges.csv \
  -o results/ \
  --avail_memory 8
```

### Input File Format For Ranges File (ranges.csv)
```
lower,upper
600,700
800,900
1000,1500
```

### Full Example

Processes with 5 variants per peptide, 2 missed cleavages, mass range clamped to 500–5000 Da, parallelized across 16 CPUs with 32 GB RAM and larger dedup buckets, compressed output. A second run on the same graphs can pass `./output/graphs.pgarena` as `-g` to skip the conversion.

```
bpcsr_to_fasta \
  -g proteins.bpcsr \
  -q target_ranges.csv \
  -o ./output \
  -z \
  -v 5 \
  -c 2 \
  -l 500 \
  -u 5000 \
  --avail_processors 16 \
  --avail_memory 32 \
  --bucket_size 16
```