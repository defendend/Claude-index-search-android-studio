//! Index management commands
//!
//! Commands for managing the code index:
//! - rebuild: Rebuild the index (full or partial)
//! - update: Incrementally update the index
//! - stats: Show index statistics

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use colored::Colorize;

use crate::db;
use crate::indexer;


/// File count threshold for auto-switching to sub-projects mode
const AUTO_SUB_PROJECTS_THRESHOLD: usize = 65_000;

/// Rebuild the index (full or partial)
pub fn cmd_rebuild(root: &Path, index_type: &str, index_deps: bool, no_ignore: bool, sub_projects: bool, project_type: Option<indexer::ProjectType>, verbose: bool) -> Result<()> {
    if verbose {
        std::env::set_var("AST_INDEX_VERBOSE", "1");
        eprintln!("[verbose] rebuild started for: {}", root.display());
        eprintln!("[verbose] index_type={}, index_deps={}, no_ignore={}, sub_projects={}", index_type, index_deps, no_ignore, sub_projects);
        eprintln!("[verbose] db path: {:?}", db::get_db_path(root).ok());
    }

    // Explicit sub-projects mode
    if sub_projects {
        return cmd_rebuild_sub_projects(root, index_type, index_deps, no_ignore, verbose);
    }

    // Auto-detect: if sub-projects exist and file count >= threshold, switch automatically
    if index_type == "all" {
        let t = Instant::now();
        let subs = indexer::find_sub_projects(root);
        if verbose {
            eprintln!("[verbose] find_sub_projects: {} found in {:?}", subs.len(), t.elapsed());
        }
        if subs.len() >= 2 {
            if verbose { eprintln!("[verbose] counting files (quick_file_count, limit={})...", AUTO_SUB_PROJECTS_THRESHOLD); }
            let t = Instant::now();
            let file_count = indexer::quick_file_count(root, no_ignore, AUTO_SUB_PROJECTS_THRESHOLD);
            if verbose {
                eprintln!("[verbose] quick_file_count: {} in {:?}", file_count, t.elapsed());
            }
            if file_count >= AUTO_SUB_PROJECTS_THRESHOLD {
                eprintln!(
                    "{}",
                    format!(
                        "Detected {}+ files and {} sub-projects — switching to sub-projects mode automatically",
                        AUTO_SUB_PROJECTS_THRESHOLD, subs.len()
                    ).yellow()
                );
                return cmd_rebuild_sub_projects(root, index_type, index_deps, no_ignore, verbose);
            }
        }
    }

    let start = Instant::now();

    // Acquire exclusive lock to prevent concurrent rebuilds
    if verbose {
        eprintln!("[verbose] acquiring rebuild lock...");
    }
    let t = Instant::now();
    let _lock = db::acquire_rebuild_lock(root)?;
    if verbose {
        eprintln!("[verbose] lock acquired in {:?}", t.elapsed());
    }

    // Save extra roots before deleting DB
    let saved_extra_roots = if db::db_exists(root) {
        if verbose { eprintln!("[verbose] reading extra roots from existing DB..."); }
        let old_conn = db::open_db(root)?;
        db::get_extra_roots(&old_conn).unwrap_or_default()
    } else {
        vec![]
    };

    // Delete DB file entirely to avoid WAL hangs
    if verbose { eprintln!("[verbose] deleting old DB..."); }
    let t = Instant::now();
    if let Err(e) = db::delete_db(root) {
        eprintln!("{}", format!("Warning: could not delete old index: {}", e).yellow());
        if let Ok(db_path) = db::get_db_path(root) {
            eprintln!("Cache path: {}", db_path.parent().unwrap_or(db_path.as_path()).display());
            eprintln!("Try manually removing the cache directory and re-running rebuild.");
        }
        return Err(e);
    }
    if verbose { eprintln!("[verbose] DB deleted in {:?}", t.elapsed()); }

    // Remove old kotlin-index cache dir entirely
    db::cleanup_legacy_cache();

    if verbose { eprintln!("[verbose] opening new DB..."); }
    let t = Instant::now();
    let mut conn = db::open_db(root)?;
    db::init_db(&conn)?;
    if verbose { eprintln!("[verbose] DB opened + schema created in {:?}", t.elapsed()); }

    // Restore extra roots
    if !saved_extra_roots.is_empty() {
        let roots_json = serde_json::to_string(&saved_extra_roots)?;
        conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('extra_roots', ?1)",
            [&roots_json],
        )?;
    }

    // Store no_ignore setting in database metadata
    if no_ignore {
        conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('no_ignore', '1')",
            [],
        ).ok();
        println!("{}", "Including gitignored files (build/, etc.)...".yellow());
    }

    // Detect project type — check actual platform markers for Mixed projects
    let _project_type = indexer::detect_project_type(root);
    let is_ios = indexer::has_ios_markers(root);
    let is_android = indexer::has_android_markers(root);

    match index_type {
        "all" => {
            println!("{}", "Rebuilding full index...".cyan());
            if verbose { eprintln!("[verbose] starting file walk + parse..."); }
            let t = Instant::now();
            let walk = indexer::index_directory_with_type(&mut conn, root, true, no_ignore, project_type)?;
            let mut file_count = walk.file_count;
            if verbose { eprintln!("[verbose] index_directory: {} files in {:?}", file_count, t.elapsed()); }

            // Collect module_files from primary root
            let mut all_module_files = walk.module_files;

            // Index extra roots and merge their module_files
            let extra_roots = db::get_extra_roots(&conn)?;
            for extra_root in &extra_roots {
                let extra_path = std::path::Path::new(extra_root);
                if extra_path.exists() {
                    if verbose { eprintln!("[verbose] indexing extra root: {}", extra_root); }
                    let t = Instant::now();
                    let extra_walk = indexer::index_directory(&mut conn, extra_path, true, no_ignore)?;
                    file_count += extra_walk.file_count;
                    all_module_files.extend(extra_walk.module_files);
                    if verbose { eprintln!("[verbose] extra root: {} files in {:?}", extra_walk.file_count, t.elapsed()); }
                    println!("{}", format!("Indexed {} files from extra root: {}", extra_walk.file_count, extra_root).dimmed());
                }
            }

            let t = Instant::now();
            let module_count = indexer::index_modules_from_files(&conn, root, &all_module_files)?;
            if verbose { eprintln!("[verbose] index_modules: {} modules in {:?}", module_count, t.elapsed()); }

            // Index CocoaPods/Carthage for iOS
            if is_ios {
                if verbose { eprintln!("[verbose] indexing CocoaPods/Carthage..."); }
                let t = Instant::now();
                let pkg_count = indexer::index_ios_package_managers(&conn, root, true)?;
                if verbose { eprintln!("[verbose] ios_package_managers: {} in {:?}", pkg_count, t.elapsed()); }
                if pkg_count > 0 {
                    println!("{}", format!("Indexed {} CocoaPods/Carthage deps", pkg_count).dimmed());
                }
            }

            let mut dep_count = 0;
            let mut trans_count = 0;
            let any_has_deps = is_android || extra_roots.iter().any(|r| {
                indexer::has_android_markers(std::path::Path::new(r))
            });
            if index_deps && any_has_deps {
                println!("{}", "Indexing module dependencies...".cyan());
                if verbose { eprintln!("[verbose] indexing module deps..."); }
                let t = Instant::now();
                dep_count = indexer::index_module_dependencies(&mut conn, root, &all_module_files, true)?;
                if verbose { eprintln!("[verbose] module_deps: {} in {:?}", dep_count, t.elapsed()); }
                let t = Instant::now();
                trans_count = indexer::build_transitive_deps(&mut conn, true)?;
                if verbose { eprintln!("[verbose] transitive_deps: {} in {:?}", trans_count, t.elapsed()); }
            }

            // Frontend-specific: .d.ts from node_modules
            let mut dts_count = 0;
            if root.join("node_modules").exists() {
                if verbose { eprintln!("[verbose] indexing .d.ts from node_modules..."); }
                let t = Instant::now();
                dts_count = indexer::index_node_modules_dts(&mut conn, root, true)?;
                if verbose { eprintln!("[verbose] node_modules .d.ts: {} files in {:?}", dts_count, t.elapsed()); }
                if dts_count > 0 {
                    println!("{}", format!("Indexed {} .d.ts type declarations from node_modules", dts_count).dimmed());
                }
            }

            // Android-specific: XML layouts and resources
            let mut xml_count = 0;
            let mut res_count = 0;
            let mut res_usage_count = 0;
            if is_android {
                println!("{}", "Indexing XML layouts...".cyan());
                let t = Instant::now();
                xml_count = indexer::index_xml_usages(&mut conn, root, &walk.xml_layout_files, true)?;
                if verbose { eprintln!("[verbose] xml_usages: {} in {:?}", xml_count, t.elapsed()); }

                println!("{}", "Indexing resources...".cyan());
                let t = Instant::now();
                let (rc, ruc) = indexer::index_resources(&mut conn, root, &walk.res_files, true)?;
                res_count = rc;
                res_usage_count = ruc;
                if verbose { eprintln!("[verbose] resources: {} defs, {} usages in {:?}", res_count, res_usage_count, t.elapsed()); }
            }

            // iOS-specific: storyboards and assets
            let mut sb_count = 0;
            let mut asset_count = 0;
            let mut asset_usage_count = 0;
            if is_ios {
                println!("{}", "Indexing storyboards/xibs...".cyan());
                let t = Instant::now();
                sb_count = indexer::index_storyboard_usages(&mut conn, root, &walk.storyboard_files, true)?;
                if verbose { eprintln!("[verbose] storyboard_usages: {} in {:?}", sb_count, t.elapsed()); }

                println!("{}", "Indexing iOS assets...".cyan());
                let t = Instant::now();
                let (ac, auc) = indexer::index_ios_assets(&mut conn, root, &walk.xcassets_dirs, true)?;
                asset_count = ac;
                asset_usage_count = auc;
                if verbose { eprintln!("[verbose] ios_assets: {} defs, {} usages in {:?}", asset_count, asset_usage_count, t.elapsed()); }
            }

            // Print summary based on project type
            if is_android && is_ios {
                println!(
                    "{}",
                    format!(
                        "Indexed {} files, {} modules, {} deps, {} XML usages, {} resources, {} storyboard usages, {} assets",
                        file_count, module_count, dep_count, xml_count, res_count, sb_count, asset_count
                    ).green()
                );
            } else if is_ios {
                println!(
                    "{}",
                    format!(
                        "Indexed {} files, {} modules, {} storyboard usages, {} assets ({} usages)",
                        file_count, module_count, sb_count, asset_count, asset_usage_count
                    ).green()
                );
            } else if dts_count > 0 {
                println!(
                    "{}",
                    format!(
                        "Indexed {} files (+{} .d.ts), {} modules, {} deps",
                        file_count, dts_count, module_count, dep_count
                    ).green()
                );
            } else {
                println!(
                    "{}",
                    format!(
                        "Indexed {} files, {} modules, {} deps, {} transitive, {} XML usages, {} resources ({} usages)",
                        file_count, module_count, dep_count, trans_count, xml_count, res_count, res_usage_count
                    ).green()
                );
            }
        }
        "files" | "symbols" => {
            println!("{}", "Rebuilding symbols index...".cyan());
            conn.execute("DELETE FROM symbols", [])?;
            conn.execute("DELETE FROM files", [])?;
            let walk = indexer::index_directory_with_type(&mut conn, root, true, no_ignore, project_type)?;
            println!("{}", format!("Indexed {} files", walk.file_count).green());
        }
        "modules" => {
            println!("{}", "Rebuilding modules index...".cyan());
            conn.execute("DELETE FROM module_deps", [])?;
            conn.execute("DELETE FROM modules", [])?;
            let module_count = indexer::index_modules(&conn, root)?;

            if index_deps {
                println!("{}", "Indexing module dependencies...".cyan());
                let gradle_files = indexer::collect_build_files_from_db(&conn, root)?;
                let dep_count = indexer::index_module_dependencies(&mut conn, root, &gradle_files, true)?;
                println!(
                    "{}",
                    format!("Indexed {} modules, {} dependencies", module_count, dep_count).green()
                );
            } else {
                println!("{}", format!("Indexed {} modules", module_count).green());
            }
        }
        "deps" => {
            println!("{}", "Indexing module dependencies...".cyan());
            let gradle_files = indexer::collect_build_files_from_db(&conn, root)?;
            let dep_count = indexer::index_module_dependencies(&mut conn, root, &gradle_files, true)?;
            println!("{}", format!("Indexed {} dependencies", dep_count).green());
        }
        _ => {
            println!("{}", format!("Unknown index type: {}", index_type).red());
        }
    }

    eprintln!("\n{}", format!("Time: {:?}", start.elapsed()).dimmed());
    Ok(())
}

/// Rebuild index for each sub-project into a single shared DB for root
fn cmd_rebuild_sub_projects(root: &Path, _index_type: &str, _index_deps: bool, no_ignore: bool, verbose: bool) -> Result<()> {
    let start = Instant::now();

    // Acquire exclusive lock to prevent concurrent rebuilds
    if verbose { eprintln!("[verbose] sub-projects: acquiring lock..."); }
    let t = Instant::now();
    let _lock = db::acquire_rebuild_lock(root)?;
    if verbose { eprintln!("[verbose] lock acquired in {:?}", t.elapsed()); }

    let t = Instant::now();
    let sub_projects = indexer::find_sub_projects(root);
    if verbose { eprintln!("[verbose] find_sub_projects: {} in {:?}", sub_projects.len(), t.elapsed()); }
    if sub_projects.is_empty() {
        println!("{}", "No sub-projects found. Use 'rebuild' without --sub-projects.".yellow());
        return Ok(());
    }

    let total = sub_projects.len();
    println!(
        "{}",
        format!("Found {} sub-projects in {}:", total, root.display()).cyan()
    );
    for (path, pt) in &sub_projects {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!("  {} [{}]", name, pt.as_str());
    }
    println!();

    // Single DB for the whole root
    if verbose { eprintln!("[verbose] deleting old DB..."); }
    let t = Instant::now();
    if let Err(e) = db::delete_db(root) {
        eprintln!("{}", format!("Warning: could not delete old index: {}", e).yellow());
        return Err(e);
    }
    let mut conn = db::open_db(root)?;
    db::init_db(&conn)?;
    if verbose { eprintln!("[verbose] DB created in {:?}", t.elapsed()); }

    if no_ignore {
        conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('no_ignore', '1')",
            [],
        ).ok();
    }

    let mut total_files = 0;
    let mut success_count = 0;
    let mut fail_count = 0;

    for (i, (path, pt)) in sub_projects.iter().enumerate() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        println!(
            "{}",
            format!("[{}/{}] Indexing {} [{}]...", i + 1, total, name, pt.as_str()).cyan()
        );

        let t = Instant::now();
        match indexer::index_directory_scoped(&mut conn, root, path, true, no_ignore, None) {
            Ok(walk) => {
                total_files += walk.file_count;
                if verbose {
                    eprintln!("[verbose] {} — {} files in {:?}", name, walk.file_count, t.elapsed());
                }
                println!(
                    "{}",
                    format!("  {} files indexed", walk.file_count).dimmed()
                );
                success_count += 1;
            }
            Err(e) => {
                if verbose { eprintln!("[verbose] {} — FAILED in {:?}: {}", name, t.elapsed(), e); }
                println!("{}", format!("  Failed: {}", e).red());
                fail_count += 1;
            }
        }
    }

    println!();
    println!(
        "{}",
        format!(
            "Done: {} sub-projects indexed ({} files total), {} failed",
            success_count, total_files, fail_count
        ).green()
    );
    eprintln!("{}", format!("Total time: {:?}", start.elapsed()).dimmed());
    Ok(())
}

/// Incrementally update the index
pub fn cmd_update(root: &Path) -> Result<()> {
    let start = Instant::now();

    if !db::db_exists(root) {
        println!(
            "{}",
            "Index not found. Run 'ast-index rebuild' first.".red()
        );
        return Ok(());
    }

    let mut conn = db::open_db(root)?;

    println!("{}", "Checking for changes...".cyan());
    let (updated, changed, deleted) = indexer::update_directory_incremental(&mut conn, root, true)?;

    if updated == 0 && deleted == 0 {
        println!("{}", "Index is up to date.".green());
    } else {
        println!(
            "{}",
            format!(
                "Updated: {} files ({} changed, {} deleted)",
                updated + deleted,
                changed,
                deleted
            )
            .green()
        );
    }

    eprintln!("\n{}", format!("Time: {:?}", start.elapsed()).dimmed());
    Ok(())
}

/// Restore index from a .db file
pub fn cmd_restore(root: &Path, db_file: &str) -> Result<()> {
    let src = std::path::Path::new(db_file);

    if !src.exists() {
        anyhow::bail!("File not found: {}", db_file);
    }
    if !src.is_file() {
        anyhow::bail!("Not a file: {}", db_file);
    }

    let dest = db::get_db_path(root)?;
    let dest_dir = dest.parent().unwrap();
    std::fs::create_dir_all(dest_dir)?;

    // Remove existing DB files if present
    if db::db_exists(root) {
        db::delete_db(root)?;
    }

    std::fs::copy(src, &dest)?;

    // Copy WAL/SHM if they exist alongside the source
    for suffix in ["-wal", "-shm"] {
        let src_extra = src.with_extension(format!("db{}", suffix));
        if src_extra.exists() {
            let dest_extra = dest.with_extension(format!("db{}", suffix));
            std::fs::copy(&src_extra, &dest_extra)?;
        }
    }

    // Update project_root metadata to match current project
    let conn = db::open_db(root)?;
    conn.execute(
        "INSERT OR REPLACE INTO metadata (key, value) VALUES ('project_root', ?1)",
        [root.to_string_lossy().as_ref()],
    )?;

    println!("{}", format!("Restored index from: {}", db_file).green());
    println!("DB path: {}", dest.display());

    // Show quick stats
    let stats = db::get_stats(&conn)?;
    println!(
        "{}",
        format!(
            "Contains: {} files, {} symbols, {} refs",
            stats.file_count, stats.symbol_count, stats.refs_count
        ).dimmed()
    );

    Ok(())
}

/// Clear index database for current project
pub fn cmd_clear(root: &Path) -> Result<()> {
    db::delete_db(root)?;
    println!("Index cleared for {}", root.display());
    Ok(())
}

/// Show index statistics
pub fn cmd_stats(root: &Path, format: &str) -> Result<()> {
    if !db::db_exists(root) {
        println!(
            "{}",
            "Index not found. Run 'ast-index rebuild' first.".red()
        );
        return Ok(());
    }

    let conn = db::open_db(root)?;
    let stats = db::get_stats(&conn)?;
    let db_path = db::get_db_path(root)?;
    let db_size = std::fs::metadata(&db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if format == "json" {
        let result = serde_json::json!({
            "project": indexer::detect_project_type(root).as_str(),
            "stats": stats,
            "db_size_bytes": db_size,
            "db_path": db_path.display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Detect project type
    let project_type = indexer::detect_project_type(root);

    println!("{}", "Index Statistics:".bold());
    println!("  Project:    {}", project_type.as_str());
    println!("  Files:      {}", stats.file_count);
    println!("  Symbols:    {}", stats.symbol_count);
    println!("  Refs:       {}", stats.refs_count);
    println!("  Modules:    {}", stats.module_count);

    // Show Android-specific stats if relevant
    if stats.xml_usages_count > 0 || stats.resources_count > 0 {
        println!("  XML usages: {}", stats.xml_usages_count);
        println!("  Resources:  {}", stats.resources_count);
    }

    // Show iOS-specific stats if relevant
    if stats.storyboard_usages_count > 0 || stats.ios_assets_count > 0 {
        println!("  Storyboard: {}", stats.storyboard_usages_count);
        println!("  iOS assets: {}", stats.ios_assets_count);
    }

    println!("  DB size:    {:.2} MB", db_size as f64 / 1024.0 / 1024.0);
    println!("  DB path:    {}", db_path.display());

    // Show extra roots if any
    let extra_roots = db::get_extra_roots(&conn)?;
    if !extra_roots.is_empty() {
        println!("\n  Extra roots:");
        for r in &extra_roots {
            println!("    {}", r);
        }
    }

    Ok(())
}

/// Add an extra source root
pub fn cmd_add_root(root: &Path, path: &str, force: bool) -> Result<()> {
    if !db::db_exists(root) {
        println!(
            "{}",
            "Index not found. Run 'ast-index rebuild' first.".red()
        );
        return Ok(());
    }

    let abs_path = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        let cwd = std::env::current_dir()?;
        cwd.join(path).to_string_lossy().to_string()
    };

    // Overlap validation
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical_new = std::path::Path::new(&abs_path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&abs_path));

    if !force {
        if canonical_new.starts_with(&canonical_root) {
            println!("{}", format!(
                "Warning: '{}' is inside the project root '{}'. Files will be indexed twice.",
                abs_path, root.display()
            ).yellow());
            println!("Use --force to add anyway, or use directory scoping instead.");
            return Ok(());
        }
        if canonical_root.starts_with(&canonical_new) {
            println!("{}", format!(
                "Warning: '{}' is a parent of the project root. This will cause massive duplication.",
                abs_path
            ).yellow());
            println!("Use --force to add anyway.");
            return Ok(());
        }
    }

    let conn = db::open_db(root)?;
    db::add_extra_root(&conn, &abs_path)?;
    println!("{}", format!("Added source root: {}", abs_path).green());
    Ok(())
}

/// Remove an extra source root
pub fn cmd_remove_root(root: &Path, path: &str) -> Result<()> {
    if !db::db_exists(root) {
        println!(
            "{}",
            "Index not found. Run 'ast-index rebuild' first.".red()
        );
        return Ok(());
    }

    let abs_path = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        let cwd = std::env::current_dir()?;
        cwd.join(path).to_string_lossy().to_string()
    };

    let conn = db::open_db(root)?;
    if db::remove_extra_root(&conn, &abs_path)? {
        println!("{}", format!("Removed source root: {}", abs_path).green());
    } else {
        println!("{}", format!("Root not found: {}", abs_path).yellow());
    }
    Ok(())
}

/// List configured source roots
pub fn cmd_list_roots(root: &Path) -> Result<()> {
    if !db::db_exists(root) {
        println!(
            "{}",
            "Index not found. Run 'ast-index rebuild' first.".red()
        );
        return Ok(());
    }

    let conn = db::open_db(root)?;
    let extra_roots = db::get_extra_roots(&conn)?;

    println!("{}", "Source roots:".bold());
    println!("  {} (primary)", root.display());
    for r in &extra_roots {
        println!("  {}", r);
    }

    Ok(())
}

/// Execute raw SQL query against the index database (SELECT only)
pub fn cmd_query(root: &Path, sql: &str, limit: usize) -> Result<()> {
    // Security: only allow SELECT statements
    let trimmed = sql.trim();
    let upper = trimmed.to_uppercase();
    if !upper.starts_with("SELECT") && !upper.starts_with("WITH") && !upper.starts_with("EXPLAIN") {
        anyhow::bail!("Only SELECT, WITH, and EXPLAIN queries are allowed");
    }
    // Block dangerous patterns
    for keyword in &["INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "ATTACH", "DETACH", "PRAGMA"] {
        // Check that these keywords appear as statements, not inside strings
        if upper.contains(&format!(" {} ", keyword)) || upper.starts_with(&format!("{} ", keyword)) {
            anyhow::bail!("Mutation queries are not allowed (found {})", keyword);
        }
    }

    let conn = db::open_db(root)?;

    // Apply LIMIT if not already in query
    let query = if !upper.contains("LIMIT") {
        format!("{} LIMIT {}", trimmed.trim_end_matches(';'), limit)
    } else {
        trimmed.trim_end_matches(';').to_string()
    };

    let mut stmt = conn.prepare(&query)?;
    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let mut rows: Vec<serde_json::Value> = Vec::new();

    let mut result_rows = stmt.query([])?;
    while let Some(row) = result_rows.next()? {
        let mut obj = serde_json::Map::new();
        for (i, col_name) in column_names.iter().enumerate() {
            let val: serde_json::Value = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(n) => serde_json::json!(n),
                rusqlite::types::ValueRef::Real(f) => serde_json::json!(f),
                rusqlite::types::ValueRef::Text(s) => {
                    serde_json::Value::String(String::from_utf8_lossy(s).to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => {
                    serde_json::Value::String(format!("<blob {} bytes>", b.len()))
                }
            };
            obj.insert(col_name.clone(), val);
        }
        rows.push(serde_json::Value::Object(obj));
    }

    let output = serde_json::json!({
        "columns": column_names,
        "rows": rows,
        "count": rows.len(),
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Print path to the SQLite index database
pub fn cmd_db_path(root: &Path) -> Result<()> {
    let db_path = db::get_db_path(root)?;
    println!("{}", db_path.display());
    Ok(())
}

/// Show database schema (tables and columns)
pub fn cmd_schema(root: &Path) -> Result<()> {
    let conn = db::open_db(root)?;

    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '%_fts%' ORDER BY name"
    )?;

    let tables: Vec<String> = stmt.query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut schema = serde_json::Map::new();

    for table in &tables {
        let mut cols_stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let columns: Vec<serde_json::Value> = cols_stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            let col_type: String = row.get(2)?;
            let not_null: bool = row.get(3)?;
            let pk: bool = row.get(5)?;
            Ok(serde_json::json!({
                "name": name,
                "type": col_type,
                "not_null": not_null,
                "primary_key": pk,
            }))
        })?.filter_map(|r| r.ok()).collect();

        // Get row count
        let count: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM {}", table),
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        schema.insert(table.clone(), serde_json::json!({
            "columns": columns,
            "row_count": count,
        }));
    }

    println!("{}", serde_json::to_string_pretty(&serde_json::Value::Object(schema))?);
    Ok(())
}
