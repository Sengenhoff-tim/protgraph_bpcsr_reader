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
| Max variants | `-v` | `--max_vars` | 3 | Maximum sequence variants per peptide |
| Max missed cleavages | `-c` | `--max_missed_cleavages` | 3 | Maximum cleavage misses. For full digestion, this describes maximum peptide length |
| Global lower bound | `-l` | `--lower_bound` | 100 | Minimum peptide mass (Da), clamps as sanity check |
| Global upper bound | `-u` | `--upper_bound` | 10000 | Maximum peptide mass (Da), clamps as sanity check |
| Zip output | `-z` | `--zip` | false | Compress output files |

### Performance & Memory

| Argument | Default | Description |
|----------|---------|-------------|
| `--avail_processors` | 1 | CPU cores for parallel processing |
| `--avail_memory` | Required | Available RAM in GB; jobs split if exceeded |
| `--interval_bin_length` | 100.0 | Mass range bin size (Da); smaller = less memory, more overhead |
| `--job_splits` | 16 | Sub-jobs per split when memory limit hit |
| `--split_depth` | 3 | Max recursive splits (exponential); failed jobs → `logs.csv` |

### Channel Sizes (tune for memory constraints)

- `--ch_proc_in_size`: Graphs buffered during processing (default: 2)
- `--ch_proc_query_size`: Query jobs buffered (default: `avail_processors * 2`)
- `--ch_proc_out_size`: Processing batches (default: `avail_processors * 2`)
- `--ch_dedup_in_size`: Binary files buffered during dedup (default: 2)
- `--ch_dedup_out_size`: Dedup batches (default: `avail_processors * 2`)

### Intermediate File Handling

- `--hash_bits`: Intermediate file bin count as `2^N`; auto-tuned if omitted
- `--max_file_handles`: Max open files (Unix: RLIMIT_NOFILE, clamped 64–8192; Windows: 2048)

### Minimal Example
```
bpcsr_to_fasta \\
  -g proteins.bpcsr \\
  -q ranges.csv \\
  -o results/ \\
  --avail\_memory 8
```

### Input File Format For Ranges File (ranges.csv)
```
lower,upper
600,700
800,900
1000,1500
```

### Full Example

Processes with 5 variants per peptide, 2 missed cleavages, mass range clamped to 500–5000 Da, parallelized across 16 CPUs with 32 GB RAM, finer binning (50 Da), and higher buffering for throughput.

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
  --interval_bin_length 50.0 \
  --job_splits 32 \
  --split_depth 4 \
  --ch_proc_in_size 4 \
  --ch_proc_query_size 32
```