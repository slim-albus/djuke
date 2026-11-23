use colored::Colorize;
use miette::Result;
use std::path::Path;

use djuke_config::resolve_project_config_with_tool;
use djuke_core::{BuildTool, DjukeError, DjukePaths, JdkDistribution};
use djuke_jdk::resolve_jdk;

use crate::cli::CliProjectOptions;
use crate::prompts::resolve_project_options;

pub async fn handle_new(
    opts: CliProjectOptions,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let init_opts = resolve_project_options(opts).await?;
    let dest = current_dir.join(&init_opts.name);
    if dest.exists()
        && dest
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(DjukeError::Generic(format!(
            "Target directory '{}' already exists and is not empty",
            dest.display()
        ))
        .into());
    }

    println!(
        "{}",
        format!(
            "Creating new Java {} project '{}'...",
            init_opts.template, init_opts.name
        )
        .bold()
        .cyan()
    );

    let resolved_java = if init_opts.java_ver.eq_ignore_ascii_case("latest")
        || init_opts.java_ver.eq_ignore_ascii_case("lts")
    {
        let disco = djuke_jdk::DiscoClient::new();
        disco.resolve_version_alias(&init_opts.java_ver).await
    } else {
        init_opts.java_ver
    };

    let framework_version = match init_opts.template {
        djuke_templates::TemplateType::SpringBoot => {
            let central = djuke_pm::MavenCentralClient::new();
            central
                .resolve_latest_version("org.springframework.boot", "spring-boot-starter-parent")
                .await
                .ok()
        }
        djuke_templates::TemplateType::Quarkus => {
            let central = djuke_pm::MavenCentralClient::new();
            central
                .resolve_latest_version("io.quarkus.platform", "quarkus-bom")
                .await
                .ok()
        }
        djuke_templates::TemplateType::Micronaut => {
            let central = djuke_pm::MavenCentralClient::new();
            central
                .resolve_latest_version("io.micronaut.platform", "micronaut-parent")
                .await
                .ok()
        }
        _ => None,
    };

    let new_opts = djuke_templates::NewProjectOptions {
        name: init_opts.name.clone(),
        group_id: init_opts.group_id,
        template: init_opts.template,
        build_tool: init_opts.build_tool,
        java_version: resolved_java,
        distribution: init_opts.distro_choice,
        destination: dest.clone(),
        framework_version,
        initializr_dependencies: init_opts.initializr_deps,
        skip_initializr: false,
        test_framework: init_opts.test_framework,
    };

    let java_str = new_opts.java_version.clone();
    djuke_templates::generate_project(&new_opts).await?;

    let tool_enum = match init_opts.build_tool {
        djuke_templates::BuildToolChoice::GradleKts
        | djuke_templates::BuildToolChoice::GradleGroovy => djuke_core::BuildTool::Gradle,
        djuke_templates::BuildToolChoice::Maven => djuke_core::BuildTool::Maven,
    };

    let resolved_v = match tool_enum {
        djuke_core::BuildTool::Gradle => {
            djuke_build::resolve_dynamic_gradle_version(&java_str, Some(paths), true).await
        }
        djuke_core::BuildTool::Maven => {
            djuke_build::resolve_dynamic_maven_version(Some(paths), true).await
        }
    };

    match tool_enum {
        djuke_core::BuildTool::Gradle => {
            let _ = djuke_build::create_gradle_wrapper(&dest, Some(&resolved_v));
        }
        djuke_core::BuildTool::Maven => {
            let _ = djuke_build::create_maven_wrapper(&dest, Some(&resolved_v));
        }
    }

    let global_cfg =
        djuke_config::global::load_global_config(&paths.config_file()).unwrap_or_default();
    let default_distro = global_cfg.default_distribution();

    let jdk_spec = djuke_core::JdkSpec {
        distribution: init_opts.distro_choice.unwrap_or(default_distro),
        version: java_str,
    };
    if let Ok(installed_jdk) = resolve_jdk(&jdk_spec, paths, true, quiet).await {
        let _ =
            djuke_build::prime_wrapper(tool_enum, &dest, &installed_jdk.java_home, paths, quiet)
                .await;
    }

    println!(
        "{}",
        "✓ Project created and toolchain primed successfully."
            .green()
            .bold()
    );
    println!("\nNext steps:");
    println!("  cd {}", init_opts.name.bold());
    println!("  djuke run\n");

    Ok(())
}

pub async fn handle_init(
    opts: CliProjectOptions,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let init_opts = resolve_project_options(opts).await?;

    println!(
        "{}",
        format!(
            "Initializing Java {} project '{}' in current directory...",
            init_opts.template, init_opts.name
        )
        .bold()
        .cyan()
    );

    let resolved_java = if init_opts.java_ver.eq_ignore_ascii_case("latest")
        || init_opts.java_ver.eq_ignore_ascii_case("lts")
    {
        let disco = djuke_jdk::DiscoClient::new();
        disco.resolve_version_alias(&init_opts.java_ver).await
    } else {
        init_opts.java_ver
    };

    let framework_version = match init_opts.template {
        djuke_templates::TemplateType::SpringBoot => {
            let central = djuke_pm::MavenCentralClient::new();
            central
                .resolve_latest_version("org.springframework.boot", "spring-boot-starter-parent")
                .await
                .ok()
        }
        djuke_templates::TemplateType::Quarkus => {
            let central = djuke_pm::MavenCentralClient::new();
            central
                .resolve_latest_version("io.quarkus.platform", "quarkus-bom")
                .await
                .ok()
        }
        djuke_templates::TemplateType::Micronaut => {
            let central = djuke_pm::MavenCentralClient::new();
            central
                .resolve_latest_version("io.micronaut.platform", "micronaut-parent")
                .await
                .ok()
        }
        _ => None,
    };

    let new_opts = djuke_templates::NewProjectOptions {
        name: init_opts.name.clone(),
        group_id: init_opts.group_id,
        template: init_opts.template,
        build_tool: init_opts.build_tool,
        java_version: resolved_java,
        distribution: init_opts.distro_choice,
        destination: current_dir.to_path_buf(),
        framework_version,
        initializr_dependencies: init_opts.initializr_deps,
        skip_initializr: false,
        test_framework: init_opts.test_framework,
    };

    let java_str = new_opts.java_version.clone();
    djuke_templates::generate_project(&new_opts).await?;

    let tool_enum = match init_opts.build_tool {
        djuke_templates::BuildToolChoice::GradleKts
        | djuke_templates::BuildToolChoice::GradleGroovy => djuke_core::BuildTool::Gradle,
        djuke_templates::BuildToolChoice::Maven => djuke_core::BuildTool::Maven,
    };

    let resolved_v = match tool_enum {
        djuke_core::BuildTool::Gradle => {
            djuke_build::resolve_dynamic_gradle_version(&java_str, Some(paths), true).await
        }
        djuke_core::BuildTool::Maven => {
            djuke_build::resolve_dynamic_maven_version(Some(paths), true).await
        }
    };

    match tool_enum {
        djuke_core::BuildTool::Gradle => {
            let _ = djuke_build::create_gradle_wrapper(current_dir, Some(&resolved_v));
        }
        djuke_core::BuildTool::Maven => {
            let _ = djuke_build::create_maven_wrapper(current_dir, Some(&resolved_v));
        }
    }

    let global_cfg =
        djuke_config::global::load_global_config(&paths.config_file()).unwrap_or_default();
    let default_distro = global_cfg.default_distribution();

    let jdk_spec = djuke_core::JdkSpec {
        distribution: init_opts.distro_choice.unwrap_or(default_distro),
        version: java_str,
    };
    if let Ok(installed_jdk) = resolve_jdk(&jdk_spec, paths, true, quiet).await {
        let _ = djuke_build::prime_wrapper(
            tool_enum,
            current_dir,
            &installed_jdk.java_home,
            paths,
            quiet,
        )
        .await;
    }

    println!(
        "{}",
        "✓ Project initialized and toolchain primed successfully."
            .green()
            .bold()
    );
    println!("\nNext steps:");
    println!("  djuke run\n");

    Ok(())
}

pub async fn handle_setup(
    tool: Option<BuildTool>,
    distro: Option<JdkDistribution>,
    spec: Option<String>,
    current_dir: &Path,
    paths: &DjukePaths,
    quiet: bool,
) -> Result<()> {
    let mut config = resolve_project_config_with_tool(current_dir, None, tool, paths);
    if let Some(s) = spec {
        if s.contains('@') {
            let parsed: djuke_core::JdkSpec = s.parse()?;
            config.jdk_spec = parsed;
        } else {
            config.jdk_spec.version = s;
        }
    }
    if let Some(d) = distro {
        config.jdk_spec.distribution = d;
    }
    if !quiet {
        println!("{}", "Setting up project environment...".bold().cyan());
    }

    // 1. Ensure JDK installed
    let installed_jdk = resolve_jdk(&config.jdk_spec, paths, true, quiet).await?;

    // 2. Validate build tool
    let tool_str = config
        .build_tool
        .map(|t| t.to_string())
        .unwrap_or_else(|| "none".into());

    // 3. Auto-bootstrap wrapper if missing
    let mut wrapper_status = "present";
    if let Some(t) = config.build_tool {
        let has_wrapper = config.has_gradle_wrapper || config.has_maven_wrapper;
        if !has_wrapper {
            if !quiet {
                println!("No wrapper detected. Bootstrapping {} wrapper...", t);
            }
            let resolved_v = match t {
                BuildTool::Gradle => {
                    djuke_build::resolve_dynamic_gradle_version(
                        &config.jdk_spec.version,
                        Some(paths),
                        true,
                    )
                    .await
                }
                BuildTool::Maven => {
                    djuke_build::resolve_dynamic_maven_version(Some(paths), true).await
                }
            };
            if let Err(e) = djuke_build::bootstrap_wrapper(
                t,
                Some(&resolved_v),
                &config.project_root,
                &installed_jdk.java_home,
                paths,
                quiet,
            )
            .await
            {
                eprintln!("warning: Wrapper generation failed: {}", e);
                wrapper_status = "missing (generation failed)";
            } else {
                wrapper_status = "generated & primed";
            }
        }
    } else {
        wrapper_status = "none";
    }

    if config.build_tool.is_some() || config.manifest_path.is_some() {
        println!("\n{}", "Project Environment Ready:".green().bold());
        println!(
            "  • Java:       {} ({})",
            installed_jdk.version.green(),
            installed_jdk.distribution
        );
        println!("  • JAVA_HOME:  {}", installed_jdk.java_home.display());
        println!(
            "  • Build Tool: {} (wrapper: {})",
            tool_str.cyan(),
            wrapper_status
        );
        println!("\nRun '{}' to build the project.", "djuke build".bold());
    } else {
        println!("\n{}", "Environment Ready:".green().bold());
        println!(
            "  • Java:       {} ({})",
            installed_jdk.version.green(),
            installed_jdk.distribution
        );
        println!("  • JAVA_HOME:  {}", installed_jdk.java_home.display());
        println!("  • Context:    Non-project directory (global JDK primed)");
        println!("\nRun '{}' to create a new project.", "djuke new".bold());
    }

    Ok(())
}
