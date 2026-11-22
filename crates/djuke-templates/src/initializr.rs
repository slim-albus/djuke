use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use djuke_core::{DjukeError, JdkDistribution, Result};

use crate::model::{BuildToolChoice, NewProjectOptions};

pub const SPRING_INITIALIZR_URL: &str = "https://start.spring.io";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializrDependency {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
}

/// Client for interacting with the official Spring Initializr API.
pub struct SpringInitializrClient {
    client: reqwest::Client,
    base_url: String,
}

impl Default for SpringInitializrClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SpringInitializrClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("djuke-cli")
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: SPRING_INITIALIZR_URL.to_string(),
        }
    }

    /// Check if Spring Initializr API is reachable
    pub async fn is_available(&self) -> bool {
        self.client
            .head(&self.base_url)
            .send()
            .await
            .map(|r| r.status().is_success() || r.status().is_redirection())
            .unwrap_or(false)
    }

    /// Fetch available dependencies from Spring Initializr metadata API with local fallback
    pub async fn fetch_dependencies(&self) -> Vec<InitializrDependency> {
        let resp = self
            .client
            .get(&self.base_url)
            .header("Accept", "application/vnd.initializr.v2.1+json")
            .timeout(std::time::Duration::from_secs(4))
            .send()
            .await;

        if let Ok(r) = resp {
            if r.status().is_success() {
                if let Ok(json) = r.json::<serde_json::Value>().await {
                    if let Some(cats) = json
                        .get("dependencies")
                        .and_then(|d| d.get("values"))
                        .and_then(|v| v.as_array())
                    {
                        let mut res = Vec::new();
                        for cat in cats {
                            let cat_name = cat
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("General")
                                .to_string();
                            if let Some(items) = cat.get("values").and_then(|v| v.as_array()) {
                                for item in items {
                                    if let (Some(id), Some(name)) = (
                                        item.get("id").and_then(|i| i.as_str()),
                                        item.get("name").and_then(|n| n.as_str()),
                                    ) {
                                        let desc = item
                                            .get("description")
                                            .and_then(|d| d.as_str())
                                            .map(|s| s.to_string());
                                        res.push(InitializrDependency {
                                            id: id.to_string(),
                                            name: name.to_string(),
                                            description: desc,
                                            category: cat_name.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        if !res.is_empty() {
                            return res;
                        }
                    }
                }
            }
        }

        Self::fallback_dependencies()
    }

    /// Curated list of most popular, must-have dependencies shown by default
    pub fn popular_dependencies() -> Vec<InitializrDependency> {
        vec![
            InitializrDependency {
                id: "web".into(),
                name: "Spring Web".into(),
                description: Some(
                    "Build web, including RESTful APIs using Spring MVC & Apache Tomcat".into(),
                ),
                category: "Web".into(),
            },
            InitializrDependency {
                id: "devtools".into(),
                name: "Spring Boot DevTools".into(),
                description: Some(
                    "Fast application restarts, LiveReload, and dev configurations".into(),
                ),
                category: "Developer Tools".into(),
            },
            InitializrDependency {
                id: "actuator".into(),
                name: "Spring Boot Actuator".into(),
                description: Some(
                    "Production-ready features like health checks and metrics".into(),
                ),
                category: "Ops".into(),
            },
            InitializrDependency {
                id: "lombok".into(),
                name: "Lombok".into(),
                description: Some(
                    "Java annotation library to reduce boilerplate (getters, builders, logs)"
                        .into(),
                ),
                category: "Developer Tools".into(),
            },
            InitializrDependency {
                id: "data-jpa".into(),
                name: "Spring Data JPA".into(),
                description: Some(
                    "Persist SQL data with Java Persistence API using Spring Data & Hibernate"
                        .into(),
                ),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "validation".into(),
                name: "Validation".into(),
                description: Some("Bean Validation with Hibernate validator annotations".into()),
                category: "I/O".into(),
            },
            InitializrDependency {
                id: "security".into(),
                name: "Spring Security".into(),
                description: Some(
                    "Highly customizable authentication and access-control framework".into(),
                ),
                category: "Security".into(),
            },
            InitializrDependency {
                id: "postgresql".into(),
                name: "PostgreSQL Driver".into(),
                description: Some("A JDBC and R2DBC driver for PostgreSQL database".into()),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "mysql".into(),
                name: "MySQL Driver".into(),
                description: Some("MySQL JDBC and R2DBC database driver".into()),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "h2".into(),
                name: "H2 Database".into(),
                description: Some("Fast in-memory embedded SQL database for dev & testing".into()),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "thymeleaf".into(),
                name: "Thymeleaf".into(),
                description: Some(
                    "Modern server-side Java template engine for HTML web views".into(),
                ),
                category: "Web".into(),
            },
            InitializrDependency {
                id: "data-redis".into(),
                name: "Spring Data Redis".into(),
                description: Some("Advanced and fast key-value data store support".into()),
                category: "NoSQL".into(),
            },
        ]
    }

    pub fn fallback_dependencies() -> Vec<InitializrDependency> {
        vec![
            InitializrDependency {
                id: "web".into(),
                name: "Spring Web".into(),
                description: Some(
                    "Build web and RESTful applications using Spring MVC and Apache Tomcat".into(),
                ),
                category: "Web".into(),
            },
            InitializrDependency {
                id: "actuator".into(),
                name: "Spring Boot Actuator".into(),
                description: Some(
                    "Production-ready features to monitor and manage applications".into(),
                ),
                category: "Ops".into(),
            },
            InitializrDependency {
                id: "data-jpa".into(),
                name: "Spring Data JPA".into(),
                description: Some(
                    "Persist data in SQL stores with Java Persistence API & Hibernate".into(),
                ),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "postgresql".into(),
                name: "PostgreSQL Driver".into(),
                description: Some("PostgreSQL JDBC driver".into()),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "mysql".into(),
                name: "MySQL Driver".into(),
                description: Some("MySQL JDBC driver".into()),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "h2".into(),
                name: "H2 Database".into(),
                description: Some("Fast in-memory SQL database for development and testing".into()),
                category: "SQL".into(),
            },
            InitializrDependency {
                id: "security".into(),
                name: "Spring Security".into(),
                description: Some(
                    "Authentication, authorization, and protection against common attacks".into(),
                ),
                category: "Security".into(),
            },
            InitializrDependency {
                id: "validation".into(),
                name: "Validation".into(),
                description: Some("Bean Validation with Hibernate Validator".into()),
                category: "I/O".into(),
            },
            InitializrDependency {
                id: "lombok".into(),
                name: "Lombok".into(),
                description: Some(
                    "Java annotation library to reduce boilerplate code (getters, builders, logs)"
                        .into(),
                ),
                category: "Developer Tools".into(),
            },
            InitializrDependency {
                id: "devtools".into(),
                name: "Spring Boot DevTools".into(),
                description: Some(
                    "Fast application restarts, LiveReload, and dev configurations".into(),
                ),
                category: "Developer Tools".into(),
            },
            InitializrDependency {
                id: "data-redis".into(),
                name: "Spring Data Redis".into(),
                description: Some("Redis key-value data store support".into()),
                category: "NoSQL".into(),
            },
            InitializrDependency {
                id: "kafka".into(),
                name: "Spring for Apache Kafka".into(),
                description: Some(
                    "Publish, subscribe, and stream processing with Apache Kafka".into(),
                ),
                category: "Messaging".into(),
            },
            InitializrDependency {
                id: "docker-compose".into(),
                name: "Docker Compose Support".into(),
                description: Some("Container management via Docker Compose files".into()),
                category: "Developer Tools".into(),
            },
            InitializrDependency {
                id: "testcontainers".into(),
                name: "Testcontainers".into(),
                description: Some(
                    "Lightweight, throwaway containerized dependencies for unit/integration tests"
                        .into(),
                ),
                category: "Testing".into(),
            },
            InitializrDependency {
                id: "graphql".into(),
                name: "Spring for GraphQL".into(),
                description: Some("Build GraphQL applications with Spring and GraphQL Java".into()),
                category: "Web".into(),
            },
            InitializrDependency {
                id: "websocket".into(),
                name: "WebSocket".into(),
                description: Some(
                    "Build WebSocket applications with SockJS and STOMP messaging".into(),
                ),
                category: "Web".into(),
            },
            InitializrDependency {
                id: "mail".into(),
                name: "Java Mail Sender".into(),
                description: Some(
                    "Send emails using JavaMail and Spring email abstractions".into(),
                ),
                category: "I/O".into(),
            },
            InitializrDependency {
                id: "quartz".into(),
                name: "Quartz Scheduler".into(),
                description: Some(
                    "Job scheduling library for background workers and cron jobs".into(),
                ),
                category: "I/O".into(),
            },
        ]
    }

    /// Download and unpack a Spring Boot starter project directly into destination
    pub async fn generate_project(&self, dest: &Path, opts: &NewProjectOptions) -> Result<()> {
        let project_type = match opts.build_tool {
            BuildToolChoice::GradleKts => "gradle-project-kotlin",
            BuildToolChoice::GradleGroovy => "gradle-project",
            BuildToolChoice::Maven => "maven-project",
        };

        // Spring Initializr supports 17, 21, 25, 27
        let java_ver = match opts.java_version.as_str() {
            "17" | "21" | "25" | "27" => opts.java_version.as_str(),
            "latest" | "lts" => "21",
            _ => "21",
        };

        let default_deps = vec!["web".to_string(), "actuator".to_string()];
        let deps = opts
            .initializr_dependencies
            .as_ref()
            .unwrap_or(&default_deps)
            .join(",");

        let url = format!("{}/starter.zip", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("type", project_type),
                ("language", "java"),
                ("groupId", &opts.group_id),
                ("artifactId", &opts.name),
                ("name", &opts.name),
                ("packageName", &opts.group_id),
                ("javaVersion", java_ver),
                ("dependencies", &deps),
            ])
            .send()
            .await
            .map_err(|e| {
                DjukeError::Network(format!("Failed to connect to Spring Initializr: {}", e))
            })?;

        if !resp.status().is_success() {
            return Err(DjukeError::Network(format!(
                "Spring Initializr returned HTTP status {}",
                resp.status()
            )));
        }

        let zip_bytes = resp
            .bytes()
            .await
            .map_err(|e| DjukeError::Network(format!("Failed to read starter zip: {}", e)))?;

        let temp_file = tempfile::NamedTempFile::new()?;
        fs::write(temp_file.path(), &zip_bytes)?;

        // Extract to staging directory
        let staging_dir = tempfile::tempdir()?;

        djuke_archive::zip_archive::extract_zip(temp_file.path(), staging_dir.path())?;

        // Spring Initializr zips enclose contents in an artifactId directory
        let entries: Vec<_> = fs::read_dir(staging_dir.path())?
            .filter_map(|e| e.ok())
            .collect();

        let source_dir = if entries.len() == 1 && entries[0].path().is_dir() {
            entries[0].path()
        } else {
            staging_dir.path().to_path_buf()
        };

        fs::create_dir_all(dest)?;
        copy_directory_contents(&source_dir, dest)?;

        // Generate or ensure djuke.toml
        let tool_name = match opts.build_tool {
            BuildToolChoice::GradleKts | BuildToolChoice::GradleGroovy => "gradle",
            BuildToolChoice::Maven => "maven",
        };
        let distro = opts
            .distribution
            .unwrap_or_else(JdkDistribution::default_distro);

        let djuke_toml = format!(
            r#"[project]
name = "{}"

[java]
version = "{}"
distribution = "{}"

[build]
tool = "{}"
wrapper = true
"#,
            opts.name, opts.java_version, distro, tool_name
        );
        fs::write(dest.join("djuke.toml"), djuke_toml)?;

        // Ensure .editorconfig and .gitattributes exist
        if !dest.join(".editorconfig").exists() {
            let editorconfig = r#"root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 4
insert_final_newline = true
trim_trailing_whitespace = true

[*.{yml,yaml,json}]
indent_size = 2

[*.md]
trim_trailing_whitespace = false

[*.{bat,cmd}]
end_of_line = crlf
"#;
            let _ = fs::write(dest.join(".editorconfig"), editorconfig);
        }

        if !dest.join(".gitattributes").exists() {
            let gitattributes = r#"# Auto detect text files and perform LF normalization
* text=auto eol=lf

# Java and build scripts
*.java text diff=java eol=lf
*.kt text eol=lf
*.kts text eol=lf
*.gradle text eol=lf
*.xml text eol=lf
*.properties text eol=lf
*.md text eol=lf

# Wrappers
gradlew text eol=lf
mvnw text eol=lf

# Windows scripts
*.bat text eol=crlf
*.cmd text eol=crlf
"#;
            let _ = fs::write(dest.join(".gitattributes"), gitattributes);
        }

        // Ensure a welcome controller exists so localhost:8080 isn't an empty 404 Whitelabel error
        let _ = ensure_welcome_controller(dest, &opts.group_id);

        Ok(())
    }
}

fn find_app_dir(dir: &Path) -> Option<std::path::PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(res) = find_app_dir(&p) {
                    return Some(res);
                }
            } else if p
                .file_name()
                .map(|n| n.to_string_lossy().ends_with("Application.java"))
                .unwrap_or(false)
            {
                return p.parent().map(|d| d.to_path_buf());
            }
        }
    }
    None
}

fn ensure_welcome_controller(dest: &Path, group_id: &str) -> Result<()> {
    let java_base = dest.join("src").join("main").join("java");
    if !java_base.exists() {
        return Ok(());
    }

    let target_dir = find_app_dir(&java_base).unwrap_or_else(|| {
        let fallback = java_base.join(group_id.replace('.', "/"));
        let _ = fs::create_dir_all(&fallback);
        fallback
    });

    let controller_file = target_dir.join("HomeController.java");
    if !controller_file.exists() {
        let rel_pkg = target_dir
            .strip_prefix(&java_base)
            .map(|p| p.to_string_lossy().replace(['/', '\\'], "."))
            .unwrap_or_else(|_| group_id.to_string());

        let code = format!(
            r#"package {};

import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RestController;
import java.util.Map;

@RestController
public class HomeController {{

    @GetMapping("/")
    public Map<String, Object> index() {{
        return Map.of(
            "status", "UP",
            "message", "Welcome to your Spring Boot application managed by djuke!",
            "framework", "Spring Boot",
            "runtime", System.getProperty("java.version"),
            "toolchain", "djuke"
        );
    }}
}}
"#,
            rel_pkg
        );
        let _ = fs::write(controller_file, code);
    }
    Ok(())
}

fn copy_directory_contents(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory_contents(&entry.path(), &target)?;
        } else {
            let _ = fs::copy(entry.path(), target);
        }
    }
    Ok(())
}
