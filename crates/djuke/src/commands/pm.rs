use std::io::IsTerminal;
use std::path::Path;
use colored::Colorize;
use miette::Result;

use djuke_config::resolve_project_config;
use djuke_core::{BuildTool, DjukeError, DjukePaths};
use djuke_pm::{
    add_gradle_dependency, add_maven_dependency, check_outdated_dependencies,
    extract_project_dependencies, remove_gradle_dependency, remove_maven_dependency,
    render_audit_report, render_dependency_tree, render_outdated_table, AuditClient,
    DependencyScope, DependencySpec, MavenCentralClient,
};

pub fn resolve_dependency_alias(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    match lower.as_str() {
        "lombok" => "org.projectlombok:lombok".into(),
        "guava" => "com.google.guava:guava".into(),
        "jackson" | "jackson-databind" => "com.fasterxml.jackson.core:jackson-databind".into(),
        "postgres" | "postgresql" => "org.postgresql:postgresql".into(),
        "mysql" => "com.mysql:mysql-connector-j".into(),
        "h2" => "com.h2database:h2".into(),
        "sqlite" => "org.xerial:sqlite-jdbc".into(),
        "junit" | "junit5" => "org.junit.jupiter:junit-jupiter".into(),
        "mockito" => "org.mockito:mockito-core".into(),
        "assertj" => "org.assertj:assertj-core".into(),
        "slf4j" => "org.slf4j:slf4j-api".into(),
        "logback" => "ch.qos.logback:logback-classic".into(),
        "gson" => "com.google.code.gson:gson".into(),
        "commons-lang" | "commons-lang3" => "org.apache.commons:commons-lang3".into(),
        "dotenv" => "io.github.cdimascio:dotenv-java".into(),
        "javalin" => "io.javalin:javalin".into(),
        "picocli" => "info.picocli:picocli".into(),
        _ => input.to_string(),
    }
}

pub async fn handle_add(
    dependency: Option<String>,
    scope: String,
    test: bool,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let dep_input = match dependency {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => {
            let is_interactive = std::io::stdin().is_terminal();
            if is_interactive {
                let theme = dialoguer::theme::ColorfulTheme::default();
                let search_query: String = dialoguer::Input::with_theme(&theme)
                    .with_prompt("Search Maven Central for library to add")
                    .interact_text()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                let central = MavenCentralClient::new();
                let docs = central.search(&search_query, 5).await?;
                if docs.is_empty() {
                    return Err(DjukeError::Generic(format!(
                        "No artifacts found matching '{}'",
                        search_query
                    ))
                    .into());
                }
                let items: Vec<String> = docs
                    .iter()
                    .map(|d| {
                        format!(
                            "{}:{} ({})",
                            d.group_id,
                            d.artifact_id,
                            d.latest_version.as_deref().unwrap_or("-")
                        )
                    })
                    .collect();
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select dependency to add")
                    .items(&items)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                format!("{}:{}", docs[idx].group_id, docs[idx].artifact_id)
            } else {
                return Err(DjukeError::Generic(
                    "Please provide a dependency coordinate (e.g. 'guava' or 'org.postgresql:postgresql')".into(),
                )
                .into());
            }
        }
    };

    let canonical_dep = resolve_dependency_alias(&dep_input);
    let mut spec: DependencySpec = canonical_dep.parse()?;
    if test {
        spec.scope = DependencyScope::Test;
    } else {
        spec.scope = scope.parse()?;
    }

    if spec.version.is_none() {
        if !quiet {
            println!(
                "Resolving latest version for {}:{} from Maven Central...",
                spec.group_id, spec.artifact_id
            );
        }
        let central = MavenCentralClient::new();
        match central
            .resolve_latest_version(&spec.group_id, &spec.artifact_id)
            .await
        {
            Ok(v) => {
                if !quiet {
                    println!("Resolved latest version: {}", v.green().bold());
                }
                spec.version = Some(v);
            }
            Err(e) => {
                eprintln!(
                    "⚠ Could not resolve version from Maven Central: {}. Proceeding without explicit version.",
                    e
                );
            }
        }
    }

    let config = resolve_project_config(current_dir, None, paths);
    match config.build_tool {
        Some(BuildTool::Maven) => {
            let pom_path = config.project_root.join("pom.xml");
            if !pom_path.is_file() {
                return Err(DjukeError::Generic("pom.xml not found in project root".into()).into());
            }
            add_maven_dependency(&pom_path, &spec)?;
            println!("✔ Added {} to pom.xml", spec.to_string().green().bold());
        }
        Some(BuildTool::Gradle) | None => {
            let kts = config.project_root.join("build.gradle.kts");
            let groovy = config.project_root.join("build.gradle");
            let build_file = if kts.is_file() {
                kts
            } else if groovy.is_file() {
                groovy
            } else {
                return Err(DjukeError::Generic(
                    "No build.gradle or build.gradle.kts found in project root".into(),
                )
                .into());
            };
            add_gradle_dependency(&build_file, &spec)?;
            println!(
                "✔ Added {} to {}",
                spec.to_string().green().bold(),
                build_file.file_name().unwrap_or_default().to_string_lossy()
            );
        }
    }

    Ok(())
}

pub fn handle_remove(
    dependency: Option<String>,
    current_dir: &Path,
    paths: &DjukePaths,
) -> Result<()> {
    let config = resolve_project_config(current_dir, None, paths);
    let dep_str = match dependency {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => {
            let deps = extract_project_dependencies(&config.project_root)?;
            if deps.is_empty() {
                return Err(
                    DjukeError::Generic("No dependencies found in project to remove.".into()).into(),
                );
            }
            let is_interactive = std::io::stdin().is_terminal();
            if is_interactive {
                let theme = dialoguer::theme::ColorfulTheme::default();
                let items: Vec<String> = deps
                    .iter()
                    .map(|d| format!("{}:{}", d.group_id, d.artifact_id))
                    .collect();
                let idx = dialoguer::Select::with_theme(&theme)
                    .with_prompt("Select dependency to remove")
                    .items(&items)
                    .default(0)
                    .interact()
                    .map_err(|e| DjukeError::Process(format!("Prompt failed: {}", e)))?;
                items[idx].clone()
            } else {
                return Err(DjukeError::Generic("Please provide dependency to remove".into()).into());
            }
        }
    };

    let (group, artifact) = if dep_str.contains(':') {
        let parts: Vec<&str> = dep_str.split(':').collect();
        (parts[0].trim().to_string(), parts[1].trim().to_string())
    } else {
        let deps = extract_project_dependencies(&config.project_root)?;
        let matched = deps
            .iter()
            .find(|d| d.artifact_id.eq_ignore_ascii_case(&dep_str));
        if let Some(m) = matched {
            (m.group_id.clone(), m.artifact_id.clone())
        } else {
            return Err(DjukeError::Generic(format!(
                "Could not find dependency matching '{}' in project. Provide 'groupId:artifactId'.",
                dep_str
            ))
            .into());
        }
    };

    match config.build_tool {
        Some(BuildTool::Maven) => {
            let pom_path = config.project_root.join("pom.xml");
            remove_maven_dependency(&pom_path, &group, &artifact)?;
            println!("✔ Removed {}:{} from pom.xml", group, artifact);
        }
        Some(BuildTool::Gradle) | None => {
            let kts = config.project_root.join("build.gradle.kts");
            let groovy = config.project_root.join("build.gradle");
            let build_file = if kts.is_file() { kts } else { groovy };
            remove_gradle_dependency(&build_file, &group, &artifact)?;
            println!(
                "✔ Removed {}:{} from {}",
                group,
                artifact,
                build_file.file_name().unwrap_or_default().to_string_lossy()
            );
        }
    }

    Ok(())
}

pub async fn handle_search(query: String, limit: u32) -> Result<()> {
    let central = MavenCentralClient::new();
    println!(
        "{}",
        format!("Searching Maven Central for '{}'...", query)
            .bold()
            .cyan()
    );
    let docs = central.search(&query, limit).await?;
    if docs.is_empty() {
        println!("No matching artifacts found.");
    } else {
        println!(
            "{:<45} {:<15} {}",
            "ARTIFACT".bold(),
            "LATEST".bold(),
            "PACKAGING".bold()
        );
        println!(
            "{}",
            "------------------------------------------------------------------------".cyan()
        );
        for doc in &docs {
            let coord = format!("{}:{}", doc.group_id, doc.artifact_id);
            let ver = doc.latest_version.as_deref().unwrap_or("-");
            let pkg = doc.packaging.as_deref().unwrap_or("jar");
            println!("{:<45} {:<15} {}", coord.green(), ver.bold(), pkg);
        }

        if let Some(top) = docs.first() {
            let ver = top.latest_version.as_deref().unwrap_or("RELEASE");
            println!("\n{}", "Quick Snippets:".bold().cyan());
            println!(
                "  {} implementation(\"{}:{}:{}\")",
                "Gradle (Kotlin):".yellow(),
                top.group_id,
                top.artifact_id,
                ver
            );
            println!(
                "  {} implementation '{}:{}:{}'",
                "Gradle (Groovy):".yellow(),
                top.group_id,
                top.artifact_id,
                ver
            );
            println!(
                "  {} <dependency><groupId>{}</groupId><artifactId>{}</artifactId><version>{}</version></dependency>",
                "Maven (pom.xml):".yellow(),
                top.group_id,
                top.artifact_id,
                ver
            );
            println!(
                "  {} djuke add {}:{}",
                "djuke add:".yellow(),
                top.group_id,
                top.artifact_id
            );
        }
    }
    Ok(())
}

pub fn handle_tree(current_dir: &Path, paths: &DjukePaths) -> Result<()> {
    let config = resolve_project_config(current_dir, None, paths);
    let deps = extract_project_dependencies(&config.project_root)?;
    let proj_name = config
        .project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let rendered = render_dependency_tree(proj_name, &deps);
    println!("{}", rendered);
    Ok(())
}

pub async fn handle_outdated(current_dir: &Path, paths: &DjukePaths) -> Result<()> {
    let config = resolve_project_config(current_dir, None, paths);
    let deps = extract_project_dependencies(&config.project_root)?;
    if deps.is_empty() {
        println!("No dependencies found in project.");
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Checking {} dependencies against Maven Central...",
            deps.len()
        )
        .bold()
        .cyan()
    );
    let central = MavenCentralClient::new();
    let results = check_outdated_dependencies(&central, &deps).await?;
    let table = render_outdated_table(&results);
    println!("\n{}", table);
    Ok(())
}

pub async fn handle_audit(current_dir: &Path, paths: &DjukePaths) -> Result<()> {
    let config = resolve_project_config(current_dir, None, paths);
    let deps = extract_project_dependencies(&config.project_root)?;
    if deps.is_empty() {
        println!("No dependencies found in project to audit.");
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Auditing {} dependencies against OSV.dev vulnerability database...",
            deps.len()
        )
        .bold()
        .cyan()
    );
    let audit_client = AuditClient::new();
    let reports = audit_client.audit_dependencies(&deps).await?;
    let report_str = render_audit_report(&reports, deps.len());
    println!("{}", report_str);
    if !reports.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
