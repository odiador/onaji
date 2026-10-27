use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let mut parts = s.splitn(2, '=');
    let key = parts.next().unwrap_or("").trim().to_string();
    let val = parts
        .next()
        .ok_or_else(|| format!("Expected KEY=VAL format, got '{}'", s))?
        .trim()
        .to_string();
    if key.is_empty() {
        return Err(format!(
            "Environment variable name cannot be empty in '{}'",
            s
        ));
    }
    Ok((key, val))
}

#[derive(Parser)]
#[command(
    name = "onaji",
    about = "Universal reactive automation engine for software engineering workflows",
    version = "0.1.0 (protocol v0.1.0)"
)]
pub struct Cli {
    #[arg(
        short,
        long,
        default_value = "onaji.yaml",
        help = "Path to the configuration manifest"
    )]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Interactive workspace onboarding, adapter discovery, doctor check, and scaffold
    Onboard,

    /// Initialize a new onaji.yaml in the current workspace by auto-detecting project structure
    Init {
        #[arg(short, long, help = "Force overwrite if onaji.yaml already exists")]
        force: bool,
    },

    /// Start the reactive daemon watcher loop
    Watch {
        #[arg(
            short,
            long,
            default_value = "250",
            help = "Debounce duration in milliseconds"
        )]
        debounce: u64,

        #[arg(
            short = 't',
            long = "tests",
            help = "Smart Test Selection: only execute tests and their causal prerequisites"
        )]
        tests: bool,

        #[arg(
            short = 'e',
            long = "env",
            value_parser = parse_key_val,
            help = "Pass custom environment variable(s) into reactively triggered tasks (KEY=VALUE)"
        )]
        env: Vec<(String, String)>,
    },

    /// Run tasks or trigger a pipeline manually or in CI/CD environments
    Run {
        #[arg(
            short,
            long,
            help = "Optional resource ID to simulate change and run impact for"
        )]
        resource: Option<String>,

        #[arg(long, help = "Run in non-interactive CI mode with strict exit codes")]
        ci: bool,

        #[arg(
            short,
            long,
            default_value = "text",
            help = "Output formatting: text, github, or json"
        )]
        output: OutputFormat,

        #[arg(long, help = "Stop execution immediately on first task failure")]
        fail_fast: bool,

        #[arg(long, help = "Enable structural OpenTelemetry causal tracing")]
        trace: bool,

        #[arg(
            long,
            default_value = "text",
            help = "Trace export format: text or json"
        )]
        trace_format: OutputFormat,

        #[arg(long, help = "Optional file path to output execution trace")]
        trace_output: Option<PathBuf>,

        #[arg(
            long,
            help = "Execute only tasks affected by changes since a git reference (e.g. 'origin/main', 'HEAD~1')"
        )]
        since: Option<String>,

        #[arg(
            short = 't',
            long = "tests",
            help = "Filter execution to affected tests and their upstream causal prerequisites"
        )]
        tests: bool,

        #[arg(long, help = "Dispatch task execution to remote cluster workers")]
        remote: bool,

        #[arg(
            short = 'e',
            long = "env",
            value_parser = parse_key_val,
            help = "Pass custom environment variable(s) into task executions (KEY=VALUE)"
        )]
        env: Vec<(String, String)>,
    },

    /// Analyze or execute tasks affected by git diff or uncommitted working directory changes
    Affected {
        #[arg(
            long,
            default_value = "origin/main",
            help = "Git reference to compare against (e.g. 'origin/main', 'HEAD~1', 'main')"
        )]
        since: String,

        #[arg(
            long,
            default_value = "true",
            help = "Include uncommitted / staged working directory changes"
        )]
        uncommitted: bool,

        #[arg(
            short = 't',
            long = "tests",
            help = "Filter to affected tests and their upstream causal prerequisites"
        )]
        tests: bool,

        #[arg(
            long,
            help = "Execute the affected tasks instead of only printing the impact report"
        )]
        run: bool,

        #[arg(long, help = "Stop execution immediately on first task failure")]
        fail_fast: bool,

        #[arg(
            short,
            long,
            default_value = "text",
            help = "Output formatting: text, github, or json"
        )]
        output: OutputFormat,

        #[arg(
            short = 'e',
            long = "env",
            value_parser = parse_key_val,
            help = "Pass custom environment variable(s) into task executions (KEY=VALUE)"
        )]
        env: Vec<(String, String)>,
    },

    /// Explain why a task was executed or cached using causal graph telemetry
    Explain {
        #[arg(help = "Task ID to explain")]
        task: String,
    },

    /// Manage remote worker daemons and cluster nodes
    Worker {
        #[command(subcommand)]
        command: WorkerCommands,
    },

    /// Security policies, authentication tokens, and sandbox audit
    Security {
        #[command(subcommand)]
        command: SecurityCommands,
    },

    /// Release engineering, artifact signature verification, and checksums
    Release {
        #[command(subcommand)]
        command: ReleaseCommands,
    },

    /// Launch the real-time interactive web dashboard and cluster inspector
    Dashboard {
        #[arg(
            short,
            long,
            default_value_t = 8080,
            help = "HTTP port to bind the dashboard web server"
        )]
        port: u16,

        #[arg(long, help = "Automatically open the dashboard in default web browser")]
        open: bool,
    },

    /// Inspect a resource or workspace using native adapters
    Inspect {
        #[arg(
            help = "Resource ID, path, or adapter name (e.g. 'openapi', 'gradle', './backend/api/openapi.yaml')"
        )]
        target: Option<String>,
    },

    /// Validate a resource against its semantic specification
    Validate {
        #[arg(help = "Resource ID or path to validate (e.g. './backend/api/openapi.yaml')")]
        target: Option<String>,
    },

    /// Inspect and render the Semantic Automation DAG
    Graph {
        #[arg(
            short,
            long,
            default_value = "text",
            help = "Output format: text, mermaid, or json"
        )]
        format: GraphFormat,
    },

    /// Run environment, toolchain, and manifest health checks
    Doctor,

    /// Validate the manifest syntax and verify DAG invariants
    Check {
        #[arg(long, help = "Run in non-interactive CI mode with strict exit codes")]
        ci: bool,

        #[arg(
            short,
            long,
            default_value = "text",
            help = "Output formatting: text, github, or json"
        )]
        output: OutputFormat,
    },

    /// Start the JSON-RPC 2.0 Daemon server (for IDE extensions / remote clients)
    Daemon {
        #[arg(
            long,
            help = "Unix domain socket path (macOS/Linux) or Named Pipe (Windows)"
        )]
        socket: Option<PathBuf>,

        #[arg(long, default_value = "true", help = "Run over standard I/O streams")]
        stdio: bool,
    },

    /// Send a JSON-RPC request to the Onaji daemon
    Rpc {
        #[arg(help = "RPC method name (e.g. 'initialize', 'graph/get', 'pipeline/run')")]
        method: String,

        #[arg(short, long, help = "Optional JSON parameters string")]
        params: Option<String>,

        #[arg(long, help = "Optional socket path to connect to")]
        socket: Option<PathBuf>,
    },

    /// Preview which tasks and artifacts are affected by a resource change
    Impact {
        #[arg(short, long, help = "ID of the resource to simulate change for")]
        resource: String,

        #[arg(long, help = "Run in non-interactive CI mode")]
        ci: bool,

        #[arg(
            short,
            long,
            default_value = "text",
            help = "Output formatting: text, github, or json"
        )]
        output: OutputFormat,
    },

    /// Manage, search, install, and publish plugins from the Onaji Registry
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    /// Multi-workspace and monorepo management
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommands,
    },

    /// Generate shell autocompletions (bash, zsh, fish, powershell, elvish)
    Completions {
        #[arg(help = "Target shell (bash, zsh, fish, powershell, elvish)")]
        shell: clap_complete::Shell,
    },

    /// Clean cached artifacts and purge workspace execution cache
    Clean {
        #[arg(long, help = "Purge entire cache including global cache")]
        all: bool,

        #[arg(short, long, help = "Invalidate cache for a specific task ID")]
        task: Option<String>,

        #[arg(
            long,
            help = "Prune CAS storage to stay under maximum size limit (e.g. '500MB', '2GB')"
        )]
        max_size: Option<String>,
    },

    /// Manage Git pre-commit hooks for automated selective testing
    Hook {
        #[command(subcommand)]
        command: HookCommands,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum HookCommands {
    /// Install git pre-commit hook into .git/hooks/pre-commit
    Install,
    /// Uninstall git pre-commit hook
    Uninstall,
}

#[derive(Subcommand, Debug, Clone)]
pub enum WorkspaceCommands {
    /// List all registered workspaces in monorepo
    List,

    /// Validate all workspace manifests and cross-workspace dependency invariants
    Check,

    /// Render cross-workspace dependency graph
    Graph {
        #[arg(long, help = "Render global monorepo cross-workspace meta-graph")]
        global: bool,

        #[arg(short, long, help = "Target specific workspace ID")]
        workspace: Option<String>,
    },

    /// Preview cross-workspace impact from a resource change
    Impact {
        #[arg(short, long, help = "Originating workspace ID")]
        workspace: String,

        #[arg(short, long, help = "Originating resource ID")]
        resource: String,
    },

    /// Execute tasks in a specific workspace or across all monorepo workspaces
    Run {
        #[arg(
            short,
            long,
            help = "Target specific workspace ID (runs all if omitted)"
        )]
        workspace: Option<String>,

        #[arg(long, help = "Stop execution immediately on first task failure")]
        fail_fast: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum WorkerCommands {
    /// Start a worker daemon on this machine
    Start {
        #[arg(
            long,
            default_value = "0.0.0.0:9100",
            help = "Socket or network bind address"
        )]
        listen: String,

        #[arg(long, help = "Custom worker identifier name")]
        id: Option<String>,
    },

    /// Show status of the local worker daemon
    Status,

    /// List connected worker nodes in the cluster
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SecurityCommands {
    /// Verify workspace security posture, environment isolation, and sandboxing rules
    Check,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommands {
    /// Search for plugins in the Onaji Registry
    Search {
        #[arg(help = "Search query keyword")]
        query: String,
    },

    /// Show detailed metadata and releases for a plugin
    Info {
        #[arg(help = "Plugin name (e.g. '@onaji/openapi' or 'openapi')")]
        name: String,
    },

    /// Install a plugin and lock its SHA-256 integrity in onaji.lock
    Add {
        #[arg(help = "Plugin specification (e.g. '@onaji/openapi' or 'openapi@1.2.0')")]
        spec: String,
    },

    /// Remove an installed plugin from workspace
    Remove {
        #[arg(help = "Plugin name to remove")]
        name: String,
    },

    /// List all installed plugins and their pinned versions in onaji.lock
    List,

    /// Update installed plugins to their latest compatible releases
    Update,

    /// Authenticate with the Onaji Plugin Registry
    Login {
        #[arg(long, help = "Optional authentication token")]
        token: Option<String>,
    },

    /// Publish the current plugin package to the Public Plugin Registry
    Publish {
        #[arg(
            long,
            default_value = "https://registry.onaji.dev",
            help = "Registry URL"
        )]
        registry: String,

        #[arg(long, help = "Authentication token for publish")]
        token: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ReleaseCommands {
    /// Verify the cryptographic signature and SHA-256 integrity of an artifact
    Verify {
        #[arg(help = "Path to the artifact archive or binary to verify")]
        artifact: PathBuf,

        #[arg(short, long, help = "Optional path to signature file or hex signature")]
        signature: Option<String>,

        #[arg(short, long, help = "Optional Ed25519 public key hex string")]
        key: Option<String>,
    },

    /// Sign an artifact using a 32-byte Ed25519 private key
    Sign {
        #[arg(help = "Path to the artifact archive or binary to sign")]
        artifact: PathBuf,

        #[arg(
            short,
            long,
            help = "32-byte Ed25519 private signing key hex string (or path to key file)"
        )]
        key: String,

        #[arg(
            short,
            long,
            help = "Optional path to write the generated signature (.sig)"
        )]
        output: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq, Default, Debug)]
pub enum OutputFormat {
    #[default]
    Text,
    Github,
    Json,
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq, Debug)]
pub enum GraphFormat {
    Text,
    Mermaid,
    Json,
}
