//! `hibernate` — the Phase 1 command-line front end for the engine.
//!
//! ```text
//! hibernate scan ~/Projects
//! hibernate hibernate ~/Projects --select LogParser --select BrowserSnaps --dry-run
//! hibernate wake ~/Projects/BrowserSnaps
//! hibernate history
//! ```

use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use hibernate_core::cleanup::hibernate::{
    self as hib, ExecutionContext, HibernateEvent, HibernateRequest, SelectedProject,
};
use hibernate_core::cleanup::quarantine::Quarantine;
use hibernate_core::config::{AppPaths, AppState, Disposition, Settings};
use hibernate_core::format;
use hibernate_core::history::HistoryStore;
use hibernate_core::model::{Project, ProjectStatus, Safety};
use hibernate_core::scanner::{scan, ScanEvent, ScanOptions};
use hibernate_core::wake;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Parser)]
#[command(
    name = "hibernate",
    version,
    about = "Keep the project. Remove what can be rebuilt."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Copy, Clone, ValueEnum)]
enum DispositionArg {
    Trash,
    Quarantine,
    Permanent,
}

impl From<DispositionArg> for Disposition {
    fn from(d: DispositionArg) -> Self {
        match d {
            DispositionArg::Trash => Disposition::Trash,
            DispositionArg::Quarantine => Disposition::Quarantine,
            DispositionArg::Permanent => Disposition::Permanent,
        }
    }
}

#[derive(Copy, Clone, ValueEnum)]
enum ExportArg {
    Csv,
    Json,
}

#[derive(Subcommand)]
enum QuarantineAction {
    /// Show every batch with its size.
    List,
    /// Permanently delete one batch by entry id.
    Purge { entry_id: String },
}

#[derive(Subcommand)]
enum Command {
    /// Scan folders and list projects with their reclaimable space.
    Scan {
        /// Folders to scan. Defaults to the folders saved in Settings.
        roots: Vec<PathBuf>,
        /// Print machine-readable JSON instead of a report.
        #[arg(long)]
        json: bool,
        /// Skip `git status` (faster on huge trees).
        #[arg(long)]
        no_git: bool,
        /// Measurement threads (default: automatic).
        #[arg(long)]
        threads: Option<usize>,
        /// Follow symbolic links (off by default).
        #[arg(long)]
        follow_symlinks: bool,
        /// Only show projects with at least this many reclaimable megabytes.
        #[arg(long, default_value_t = 0)]
        min_mb: u64,
        /// Ignore cached sizes and measure every folder again.
        #[arg(long)]
        full: bool,
    },
    /// Show global toolchain caches (Cargo registry, npm cache, …) and how to clean them.
    Caches {
        #[arg(long)]
        json: bool,
    },
    /// Export the project table as CSV or JSON.
    Export {
        /// Folders to scan. Defaults to the folders saved in Settings.
        roots: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = ExportArg::Csv)]
        format: ExportArg,
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// List or purge quarantine batches.
    Quarantine {
        #[command(subcommand)]
        action: QuarantineAction,
    },
    /// Remove regeneratable artifacts from selected projects.
    Hibernate {
        /// Folders to scan. Defaults to the folders saved in Settings.
        roots: Vec<PathBuf>,
        /// Project name or path to hibernate. Repeatable.
        #[arg(short, long)]
        select: Vec<String>,
        /// Select every dormant, unprotected project.
        #[arg(long)]
        all_dormant: bool,
        /// Show the plan and exit without removing anything.
        #[arg(long)]
        dry_run: bool,
        /// Where removed folders go. Defaults to the Settings value (Trash).
        #[arg(long, value_enum)]
        disposition: Option<DispositionArg>,
        /// Also remove amber "review" folders such as .venv/.
        #[arg(long)]
        include_review: bool,
        /// Do not ask for confirmation.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Show (and optionally run) the commands that restore a project's dependencies.
    Wake {
        path: PathBuf,
        /// Run the commands after showing them.
        #[arg(long)]
        run: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// List previous hibernate runs.
    History {
        #[arg(long)]
        json: bool,
    },
    /// Move quarantined folders from a history entry back into place.
    Restore { entry_id: String },
    /// Mark a project as protected (or unprotect it with --off).
    Protect {
        path: PathBuf,
        #[arg(long)]
        off: bool,
    },
    /// Print where settings, history and quarantine live.
    Paths,
}

fn main() {
    let cli = Cli::parse();
    let paths = AppPaths::default_locations();
    if let Err(err) = paths.ensure() {
        eprintln!(
            "warning: cannot create data folder {}: {err}",
            paths.data_dir.display()
        );
    }
    let settings = Settings::load(&paths);
    let mut state = AppState::load(&paths);
    state.prune(Utc::now());

    let code = match cli.command {
        Command::Scan {
            roots,
            json,
            no_git,
            threads,
            follow_symlinks,
            min_mb,
            full,
        } => {
            let roots = roots_or_saved(roots, &settings);
            let mut opts = settings.scan_options();
            opts.inspect_git = !no_git && opts.inspect_git;
            if let Some(t) = threads {
                opts.max_concurrency = t;
            }
            opts.follow_symlinks = follow_symlinks || opts.follow_symlinks;
            let cache = load_cache(&paths, &settings, full);
            opts.tree_cache = cache.clone();
            let code = cmd_scan(&roots, &opts, &state, json, min_mb);
            save_cache(&paths, cache);
            code
        }
        Command::Caches { json } => {
            let cancel = Arc::new(AtomicBool::new(false));
            install_ctrl_c(cancel.clone());
            let caches = hibernate_core::caches::measure_caches(&cancel);
            if json {
                println!("{}", serde_json::to_string_pretty(&caches).unwrap());
            } else {
                println!("Global toolchain caches (never removed by hibernate)\n");
                for c in caches.iter().filter(|c| c.exists) {
                    println!(
                        "{:<28}{:>10}   {}",
                        c.label,
                        format::bytes(c.bytes),
                        c.path.display()
                    );
                    if let Some(cmd) = &c.clean_command {
                        println!("{:<28}{:>10}   clean: {cmd}", "", "");
                    }
                }
                let total: u64 = caches.iter().map(|c| c.bytes).sum();
                println!("\n{} in caches total", format::bytes(total));
            }
            0
        }
        Command::Export {
            roots,
            format: fmt,
            out,
        } => {
            let roots = roots_or_saved(roots, &settings);
            let mut opts = settings.scan_options();
            let cache = load_cache(&paths, &settings, false);
            opts.tree_cache = cache.clone();
            let result = run_scan(&roots, &opts, &state, out.is_some());
            save_cache(&paths, cache);
            let text = hibernate_core::export::render(
                &result.projects,
                match fmt {
                    ExportArg::Csv => hibernate_core::export::ExportFormat::Csv,
                    ExportArg::Json => hibernate_core::export::ExportFormat::Json,
                },
            );
            match out {
                Some(path) => match std::fs::write(&path, text) {
                    Ok(()) => {
                        println!(
                            "Wrote {} projects to {}",
                            result.projects.len(),
                            path.display()
                        );
                        0
                    }
                    Err(err) => {
                        eprintln!("error: {}: {err}", path.display());
                        1
                    }
                },
                None => {
                    print!("{text}");
                    0
                }
            }
        }
        Command::Quarantine { action } => {
            let q = Quarantine::new(&paths.quarantine_dir);
            match action {
                QuarantineAction::List => {
                    let batches = q.list_batches();
                    if batches.is_empty() {
                        println!("Quarantine is empty.");
                    }
                    for b in &batches {
                        println!(
                            "{}   {}   {:>10}   {}",
                            b.entry_id,
                            b.date,
                            format::bytes(b.bytes),
                            b.projects.join(", ")
                        );
                    }
                    0
                }
                QuarantineAction::Purge { entry_id } => match q.purge_batch(&entry_id) {
                    Ok(bytes) => {
                        println!("Purged {entry_id} ({})", format::bytes(bytes));
                        0
                    }
                    Err(err) => {
                        eprintln!("error: {err}");
                        1
                    }
                },
            }
        }
        Command::Hibernate {
            roots,
            select,
            all_dormant,
            dry_run,
            disposition,
            include_review,
            yes,
        } => {
            let roots = roots_or_saved(roots, &settings);
            let disposition = disposition.map(Into::into).unwrap_or(settings.disposition);
            cmd_hibernate(
                &paths,
                &settings,
                &mut state,
                &roots,
                &select,
                all_dormant,
                dry_run,
                disposition,
                include_review,
                yes,
            )
        }
        Command::Wake { path, run, yes } => cmd_wake(&settings, &state, &path, run, yes),
        Command::History { json } => cmd_history(&paths, json),
        Command::Restore { entry_id } => cmd_restore(&paths, &entry_id),
        Command::Protect { path, off } => {
            let canonical = std::fs::canonicalize(&path).unwrap_or(path);
            state.set_protected(&canonical, !off);
            match state.save(&paths) {
                Ok(()) => {
                    println!(
                        "{} {}",
                        if off { "Unprotected" } else { "Protected" },
                        canonical.display()
                    );
                    0
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    1
                }
            }
        }
        Command::Paths => {
            println!("data:       {}", paths.data_dir.display());
            println!("settings:   {}", paths.settings_file.display());
            println!("state:      {}", paths.state_file.display());
            println!("history:    {}", paths.history_file.display());
            println!("quarantine: {}", paths.quarantine_dir.display());
            0
        }
    };
    std::process::exit(code);
}

fn load_cache(
    paths: &AppPaths,
    settings: &Settings,
    full: bool,
) -> Option<Arc<Mutex<hibernate_core::scanner::TreeCache>>> {
    if !settings.incremental_scans {
        return None;
    }
    let mut cache = hibernate_core::scanner::TreeCache::load(&paths.tree_cache_file);
    if full {
        cache.clear();
    }
    Some(Arc::new(Mutex::new(cache)))
}

fn save_cache(paths: &AppPaths, cache: Option<Arc<Mutex<hibernate_core::scanner::TreeCache>>>) {
    if let Some(cache) = cache {
        if let Ok(c) = cache.lock() {
            if let Err(err) = c.save(&paths.tree_cache_file) {
                eprintln!("warning: could not save size cache: {err}");
            }
        }
    }
}

fn roots_or_saved(roots: Vec<PathBuf>, settings: &Settings) -> Vec<PathBuf> {
    if roots.is_empty() {
        if settings.scan_roots.is_empty() {
            eprintln!("error: no folders given and none saved in Settings");
            std::process::exit(2);
        }
        settings.scan_roots.clone()
    } else {
        roots
    }
}

fn run_scan(
    roots: &[PathBuf],
    opts: &ScanOptions,
    state: &AppState,
    progressive: bool,
) -> hibernate_core::scanner::ScanResult {
    let cancel = Arc::new(AtomicBool::new(false));
    install_ctrl_c(cancel.clone());
    let stderr = Mutex::new(io::stderr());
    scan(roots, opts, state, &cancel, &|event| {
        let mut err = stderr.lock().unwrap();
        match event {
            ScanEvent::Discovered {
                name, discovered, ..
            } => {
                if progressive {
                    let _ = write!(err, "\r\x1b[2Kfound {discovered} projects… {name}");
                }
            }
            ScanEvent::Scanned {
                scanned,
                discovered,
                project,
            } => {
                if progressive {
                    let _ = write!(
                        err,
                        "\r\x1b[2Kmeasured {scanned}/{discovered}… {} ({} reclaimable)",
                        project.name,
                        format::bytes(project.reclaimable_bytes)
                    );
                }
            }
            ScanEvent::Warning { path, message } => {
                let _ = writeln!(
                    err,
                    "\r\x1b[2Kwarning: {}{message}",
                    path.map(|p| format!("{}: ", p.display()))
                        .unwrap_or_default()
                );
            }
            ScanEvent::Finished { .. } | ScanEvent::Cancelled { .. } => {
                if progressive {
                    let _ = write!(err, "\r\x1b[2K");
                }
            }
            ScanEvent::Started { .. } => {}
        }
        let _ = err.flush();
    })
}

fn cmd_scan(
    roots: &[PathBuf],
    opts: &ScanOptions,
    state: &AppState,
    json: bool,
    min_mb: u64,
) -> i32 {
    let result = run_scan(roots, opts, state, !json);
    let now = Utc::now();
    if json {
        let out = serde_json::json!({
            "projects": result.projects,
            "summary": result.summary,
            "cancelled": result.cancelled,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return if result.cancelled { 130 } else { 0 };
    }

    let visible: Vec<&Project> = result
        .projects
        .iter()
        .filter(|p| p.reclaimable_bytes + p.review_bytes >= min_mb * 1_000_000)
        .collect();
    for p in &visible {
        print_project(p, now);
    }
    let s = &result.summary;
    println!("{}", "─".repeat(64));
    println!(
        "{} projects · {} total · {} safely reclaimable ({}%){}",
        s.project_count,
        format::bytes(s.total_bytes),
        format::bytes(s.reclaimable_bytes),
        if s.total_bytes > 0 {
            s.reclaimable_bytes * 100 / s.total_bytes
        } else {
            0
        },
        if s.review_bytes > 0 {
            format!(" · {} more needs review", format::bytes(s.review_bytes))
        } else {
            String::new()
        }
    );
    if !s.by_category.is_empty() {
        println!();
        println!("Reclaimable storage by category");
        for c in &s.by_category {
            println!("  {:<22}{:>10}", c.category.label(), format::bytes(c.bytes));
        }
    }
    println!();
    println!(
        "Scanned in {:.1}s{}",
        s.duration_ms as f64 / 1000.0,
        if s.cache_hits > 0 {
            format!(
                " · {} folder sizes reused from the previous scan",
                s.cache_hits
            )
        } else {
            String::new()
        }
    );
    if result.cancelled {
        println!("(scan cancelled — results are partial)");
        130
    } else {
        0
    }
}

fn print_project(p: &Project, now: chrono::DateTime<Utc>) {
    let status = match p.status {
        ProjectStatus::Active => "Active",
        ProjectStatus::Dormant => "Dormant",
        ProjectStatus::Hibernated => "Hibernated",
        ProjectStatus::Protected => "Protected",
    };
    let mut label = p.primary_label();
    if p.stacks.len() > 1 {
        label = p
            .stacks
            .iter()
            .map(|s| s.label())
            .collect::<Vec<_>>()
            .join(" + ");
    }
    let activity = p
        .last_activity_at
        .map(format::relative)
        .unwrap_or_else(|| "unknown".into());
    println!("{}", p.name);
    println!("  {}", p.path.display());
    println!("  {label} · {status} · last active {activity}");
    if let Some(git) = &p.git {
        if git.is_repo {
            println!(
                "  Git: {}{}",
                git.state.label(),
                git.branch
                    .as_ref()
                    .map(|b| format!(" ({b})"))
                    .unwrap_or_default()
            );
        }
    }
    println!("  Total:       {}", format::bytes(p.total_bytes));
    println!("  Reclaimable: {}", format::bytes(p.reclaimable_bytes));
    for a in &p.artifacts {
        let tag = match a.safety {
            Safety::Safe => "safe",
            Safety::Review => "review",
            Safety::Protected => "protected",
        };
        println!(
            "    {:<28}{:>10}   {tag}",
            a.relative_path,
            format::bytes(a.bytes)
        );
    }
    for w in &p.safety_reasons {
        println!("  ! {w}");
    }
    println!();
    let _ = now;
}

#[allow(clippy::too_many_arguments)]
fn cmd_hibernate(
    paths: &AppPaths,
    settings: &Settings,
    state: &mut AppState,
    roots: &[PathBuf],
    select: &[String],
    all_dormant: bool,
    dry_run: bool,
    disposition: Disposition,
    include_review: bool,
    yes: bool,
) -> i32 {
    if select.is_empty() && !all_dormant {
        eprintln!("error: choose projects with --select <name|path> or --all-dormant");
        return 2;
    }
    let mut opts = settings.scan_options();
    let cache = load_cache(paths, settings, false);
    opts.tree_cache = cache.clone();
    let result = run_scan(roots, &opts, state, true);
    save_cache(paths, cache);
    if result.cancelled {
        eprintln!("scan cancelled");
        return 130;
    }
    let projects = result.projects;

    let mut selection: Vec<SelectedProject> = Vec::new();
    for s in select {
        let wanted = Path::new(s);
        let canonical = std::fs::canonicalize(wanted).ok();
        let matches: Vec<&Project> = projects
            .iter()
            .filter(|p| p.name == *s || canonical.as_deref() == Some(p.path.as_path()))
            .collect();
        if matches.is_empty() {
            eprintln!("error: no project named or at {s}");
            return 2;
        }
        for p in matches {
            selection.push(SelectedProject {
                project_id: p.id.clone(),
                artifact_paths: None,
            });
        }
    }
    if all_dormant {
        for p in projects.iter().filter(|p| {
            p.status == ProjectStatus::Dormant && !p.protected && p.ignored_until.is_none()
        }) {
            if !selection.iter().any(|s| s.project_id == p.id) {
                selection.push(SelectedProject {
                    project_id: p.id.clone(),
                    artifact_paths: None,
                });
            }
        }
    }

    let request = HibernateRequest {
        selection,
        include_review,
    };
    let plan = hib::plan(&projects, &request, disposition);
    for name in &plan.skipped_protected {
        println!("skipping protected project {name}");
    }
    if plan.is_empty() {
        println!("Nothing to hibernate.");
        return 0;
    }

    println!(
        "Hibernate {} project{}?",
        plan.projects.len(),
        if plan.projects.len() == 1 { "" } else { "s" }
    );
    println!();
    println!("Estimated recovery: {}", format::bytes(plan.total_bytes));
    println!(
        "Safe items:         {} folders, {} files",
        format::count(plan.folder_count),
        format::count(plan.file_count)
    );
    println!("Review items:       {}", plan.review_count);
    println!("Protected files:    untouched");
    println!("Removed folders go to: {}", disposition.label());
    println!();
    for p in &plan.projects {
        println!("{:<32}{:>10}", p.name, format::bytes(p.bytes));
        for a in &p.artifacts {
            println!("  {:<30}{:>10}", a.relative_path, format::bytes(a.bytes));
        }
        for a in &p.skipped_review {
            println!(
                "  {:<30}{:>10}   (needs review, use --include-review)",
                a.relative_path,
                format::bytes(a.bytes)
            );
        }
        for w in &p.warnings {
            println!("  ! {w}");
        }
    }
    println!();
    if dry_run {
        println!("Dry run: nothing was removed.");
        return 0;
    }
    if !yes && !confirm(&format!("Hibernate {} projects", plan.projects.len())) {
        println!("Cancelled.");
        return 1;
    }

    let quarantine = Quarantine::new(&paths.quarantine_dir);
    let rules = settings.rule_set();
    let ctx = ExecutionContext {
        scan_roots: &result.summary.roots,
        rules: &rules,
        projects: &projects,
        quarantine: &quarantine,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    install_ctrl_c(cancel.clone());
    let entry = hib::execute(&plan, &ctx, &cancel, &|event| match event {
        HibernateEvent::ProjectStarted { name, .. } => println!("{name}"),
        HibernateEvent::ArtifactStarted { relative_path, .. } => {
            print!("  cleaning {relative_path} …");
            let _ = io::stdout().flush();
        }
        HibernateEvent::ArtifactFinished {
            bytes_recovered,
            outcome,
            ..
        } => {
            if outcome.is_error() {
                println!(" ✗ {}", describe_outcome(&outcome));
            } else {
                println!(
                    " ✓ {} {}",
                    format::bytes(bytes_recovered),
                    describe_outcome(&outcome)
                );
            }
        }
        HibernateEvent::ProjectFinished {
            bytes_recovered,
            completed,
            total,
            ..
        } => {
            println!(
                "  {} recovered  ({completed}/{total})",
                format::bytes(bytes_recovered)
            );
        }
        _ => {}
    });

    let mut history = HistoryStore::load(paths);
    hib::record_hibernations(&entry, state);
    history.push(entry.clone());
    if let Err(err) = history.save(paths) {
        eprintln!("warning: could not save history: {err}");
    }
    if let Err(err) = state.save(paths) {
        eprintln!("warning: could not save state: {err}");
    }

    println!();
    println!(
        "Cleanup {}",
        if entry.cancelled {
            "cancelled"
        } else {
            "complete"
        }
    );
    println!("{} recovered", format::bytes(entry.total_recovered));
    println!("{} projects hibernated", entry.project_count);
    println!("{} errors", entry.error_count);
    if !entry.projects.is_empty() {
        println!();
        println!("Largest savings:");
        for (name, bytes) in entry.largest_savings(5) {
            println!("  {:<30}{:>10}", name, format::bytes(bytes));
        }
    }
    if entry.error_count > 0 {
        println!();
        for p in &entry.projects {
            for a in &p.artifacts {
                if a.outcome.is_error() {
                    println!(
                        "  {}{}: {}",
                        p.name,
                        a.relative_path
                            .trim_end_matches('/')
                            .to_string()
                            .replace(a.relative_path.as_str(), &format!("/{}", a.relative_path)),
                        describe_outcome(&a.outcome)
                    );
                }
            }
        }
    }
    if entry.error_count > 0 {
        1
    } else {
        0
    }
}

fn describe_outcome(outcome: &hibernate_core::history::ArtifactOutcome) -> String {
    use hibernate_core::history::ArtifactOutcome::*;
    match outcome {
        Trashed => "moved to Trash".into(),
        Quarantined { .. } => "quarantined".into(),
        Deleted => "deleted".into(),
        Restored => "restored".into(),
        PartiallyDeleted { failed } => format!(
            "{} item(s) could not be removed: {}",
            failed.len(),
            failed
                .iter()
                .take(3)
                .map(|f| format!("{} ({})", f.path.display(), f.error))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        Failed { error } => format!("failed: {error}"),
        Skipped { reason } => format!("skipped: {reason}"),
    }
}

fn cmd_wake(settings: &Settings, state: &AppState, path: &Path, run: bool, yes: bool) -> i32 {
    let canonical = match std::fs::canonicalize(path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("error: {}: {err}", path.display());
            return 2;
        }
    };
    let mut opts = settings.scan_options();
    opts.inspect_git = false;
    let result = run_scan(std::slice::from_ref(&canonical), &opts, state, false);
    let Some(project) = result.projects.iter().find(|p| p.path == canonical) else {
        eprintln!("error: {} is not a recognised project", canonical.display());
        return 2;
    };
    let Some(plan) = wake::plan_for(project) else {
        eprintln!("error: no wake command known for this project");
        return 2;
    };
    println!("Wake {}", project.name);
    println!();
    if let Some(pm) = &plan.package_manager {
        println!("Detected package manager: {pm}");
    }
    println!("Commands:");
    for step in &plan.steps {
        println!("  {}", step.display);
    }
    println!();
    println!("These run inside: {}", plan.cwd.display());
    for note in &plan.notes {
        println!("  {note}");
    }
    for tool in &plan.missing_tools {
        println!("  ! `{tool}` was not found on PATH");
    }
    if !run {
        println!();
        println!("Add --run to execute.");
        return 0;
    }
    if !yes && !confirm("Run these commands") {
        println!("Cancelled.");
        return 1;
    }
    let cancel = Arc::new(AtomicBool::new(false));
    install_ctrl_c(cancel.clone());
    for step in &plan.steps {
        println!();
        println!("$ {}", step.display);
        let started = std::time::Instant::now();
        match wake::run_step(step, &plan.cwd, &cancel, &mut |line| println!("{line}")) {
            Ok(0) => println!(
                "Completed in {:.1} seconds.",
                started.elapsed().as_secs_f64()
            ),
            Ok(code) => {
                eprintln!("command exited with status {code}");
                return code.clamp(1, 255);
            }
            Err(err) => {
                eprintln!("could not start {}: {err}", step.program);
                return 1;
            }
        }
    }
    0
}

fn cmd_history(paths: &AppPaths, json: bool) -> i32 {
    let history = HistoryStore::load(paths);
    if json {
        println!("{}", serde_json::to_string_pretty(&history).unwrap());
        return 0;
    }
    if history.entries.is_empty() {
        println!("No cleanups yet.");
        return 0;
    }
    for e in &history.entries {
        println!(
            "{}   [{}]",
            e.finished_at
                .with_timezone(&chrono::Local)
                .format("%B %-d, %Y %H:%M"),
            e.id
        );
        println!(
            "{} projects hibernated · {} recovered · {} errors · {}{}",
            e.project_count,
            format::bytes(e.total_recovered),
            e.error_count,
            e.disposition.label(),
            if e.restorable() { " · restorable" } else { "" }
        );
        for (name, bytes) in e.largest_savings(5) {
            println!("  {:<30}{:>10}", name, format::bytes(bytes));
        }
        println!();
    }
    0
}

fn cmd_restore(paths: &AppPaths, entry_id: &str) -> i32 {
    let mut history = HistoryStore::load(paths);
    let quarantine = Quarantine::new(&paths.quarantine_dir);
    let Some(entry) = history.get_mut(entry_id) else {
        eprintln!("error: no history entry {entry_id}");
        return 2;
    };
    if !entry.restorable() {
        println!("Nothing in quarantine for this entry.");
        return 0;
    }
    let (restored, errors) = hib::restore_entry(entry, &quarantine);
    let mut state = AppState::load(paths);
    for p in &entry.projects {
        state.hibernations.remove(&p.path);
    }
    let _ = state.save(paths);
    if let Err(err) = history.save(paths) {
        eprintln!("warning: could not save history: {err}");
    }
    println!("Restored {restored} folder(s).");
    for e in &errors {
        eprintln!("  {e}");
    }
    if errors.is_empty() {
        0
    } else {
        1
    }
}

fn confirm(prompt: &str) -> bool {
    print!("{prompt}? [y/N] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim(), "y" | "Y" | "yes")
}

fn install_ctrl_c(cancel: Arc<AtomicBool>) {
    // First Ctrl-C asks the engine to stop at the next safe point; a second
    // one terminates the process.
    let _ = ctrlc::set_handler(move || {
        if cancel.swap(true, Ordering::Relaxed) {
            std::process::exit(130);
        }
        eprintln!("\ncancelling… (press Ctrl-C again to force quit)");
    });
}
