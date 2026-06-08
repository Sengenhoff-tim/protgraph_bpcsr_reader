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
        value_name = "U8",
        default_value_t = 3,
        help = "Maximum variants per peptide. Defaults to 3. Increases results exponentially"
    )]
    pub max_vars: u8,

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
        help = "Available memory in GB. When estimated usage exceeds this, jobs are split and rescheduled. Splitting should be avoided. See documentation for details."
    )]
    pub avail_memory: usize,

    #[arg(
        long = "ch_proc_in_size",
        value_name = "U64",
        help = "Amount of graphs read to memory concurrently during graph processing. Defaults to 2."
    )]
    pub ch_proc_in_size: Option<usize>,

    #[arg(
        long = "ch_proc_query_size",
        value_name = "U64",
        help = "Amount of jobs loaded to memory concurrently during graph processing. Defaults to avail_cpus*2."
    )]
    pub ch_proc_query_size: Option<usize>,

    #[arg(
        long = "ch_proc_out_size",
        value_name = "U64",
        help = "Amount of batches during processing. Defaults to avail_cpus*2."
    )]
    pub ch_proc_out_size: Option<usize>,

    #[arg(
        long = "ch_dedup_in_size",
        value_name = "U64",
        help = "Amount of binary graph processing output files loaded to memory concurrently during deduplication. Defaults to 2."
    )]
    pub ch_dedup_in_size: Option<usize>,

    #[arg(
        long = "ch_dedup_out_size",
        value_name = "U64",
        help = "Amount of sequence-metadata pairs loaded to memory concurrently during deduplication. Defaults to avail_cpus*2."
    )]
    pub ch_dedup_out_size: Option<usize>,

    // intermediate file options
    #[arg(
        long = "hash_bits",
        value_name = "U8",
        help = "Creates 2^hash_bits intermediate bins as files. Only used when deduplicating. Gives rough control over intermediate file size. Defaults to auto-tuned value based on max file handles."
    )]
    pub hash_bits: Option<u8>,

    #[arg(
        long = "max_file_handles",
        value_name = "U8",
        help = "Maximum file handles for intermediate files. Only used when deduplicating. With Unix, defaults to RLIMIT_NOFILE, clamped between 64 and 8192. With Windows, defaults to 2048"
    )]
    pub max_handles: Option<u32>,

    // scheduling constraints
    #[arg(
        long = "interval_bin_length",
        value_name = "F64",
        default_value_t = 100.0,
        help = "Interval bin size in Da. Smaller bins = less memory required, less frequent job splits, but more overhead."
    )]
    pub interval_bin_size: f64,

    #[arg(
        long = "job_splits",
        value_name = "U64",
        default_value_t = 16,
        help = "Number of sub-jobs created per split when memory limit reached."
    )]
    pub job_splits: u64,

    #[arg(
        long = "split_depth",
        value_name = "U8",
        default_value_t = 3,
        help = "Maximum times a job can be recursively split (increases exponentially; failed jobs saved to 'logs.csv')."
    )]
    pub job_split_depth: u8,
}
