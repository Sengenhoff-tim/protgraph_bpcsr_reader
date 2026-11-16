use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Cli {
    // i/o args
    #[arg(
        short = 'g',
        long = "graphs",
        value_name = "PATH",
        help = "Path to protein graph file (.bpcsr from ProtGraph)"
    )]
    pub graph_input_path: PathBuf,

    #[arg(
        short = 'q',
        long = "queries",
        value_name = "PATH",
        help = "Path to query CSV file. Format: 'lower,upper\\n600,700\\n...' (inclusive ranges in Da)"
    )]
    pub query_input_path: PathBuf,

    #[arg(
        short = 'o',
        long = "outdir",
        value_name = "PATH",
        help = "Output directory"
    )]
    pub outdir_path: PathBuf,

    #[arg(short = 'z', long = "zip", value_name = "U8", help = "Zip output")]
    pub zip: bool,

    // functional arguments
    #[arg(
        short = 'v',
        long = "max_vars",
        value_name = "U16",
        default_value_t = 3,
        help = "Maximum variants per peptide. Defaults to 3."
    )]
    pub max_vars: u16,

    #[arg(
        short = 'c',
        long = "max_missed_cleavages",
        value_name = "U16",
        default_value_t = 3,
        help = "Maximum missed cleavages per peptide. Defaults to 3."
    )]
    pub max_cleavages: u16,

    #[arg(
        short = 'l',
        long = "lower_bound",
        value_name = "I64",
        default_value_t = 100,
        help = "Global minimum peptide weight in Da. Defaults to 100."
    )]
    pub lower_bound: i64,

    #[arg(
        short = 'u',
        long = "upper_bound",
        value_name = "I64",
        default_value_t = 10000,
        help = "Global maximum peptide weight in Da. Defaults to 10000."
    )]
    pub upper_bound: i64,

    #[arg(
        long = "bucket_size",
        value_name = "U8",
        default_value_t = 8,
        value_parser = clap::value_parser!(u8).range(1..),
        help = "Entries per bucket of the in-memory dedup tables. Larger buckets evict less but make lookups slower. Defaults to 8."
    )]
    pub bucket_size: u8,

    // memory/processing constraints
    #[arg(
        long = "avail_processors",
        value_name = "U64",
        default_value_t = 1,
        help = "Number of available processors"
    )]
    pub avail_processors: usize,

    #[arg(
        long = "avail_memory",
        value_name = "U64",
        help = "Available memory in GB. During traversal 60% is used for the in-memory dedup tables (results that do not fit are spilled to disk), 70% for the final dedup of spilled files."
    )]
    pub avail_memory: usize,

    #[arg(
        long = "max_file_handles",
        value_name = "U32",
        help = "Maximum number of spill files open at once during traversal, over all writers. Defaults to 80% of RLIMIT_NOFILE, clamped between 64 and 8192 (Windows: 2048)."
    )]
    pub max_handles: Option<u32>,
}
