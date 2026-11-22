use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use djuke_core::{DjukeError, Result};

use crate::central::MavenCentralClient;

const MAVEN_CENTRAL_REPO_URL: &str = "https://repo1.maven.org/maven2";

/// Parsed script directives from inline comments in a Java source file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptDirectives {
    pub dependencies: Vec<String>,
    pub java_version: Option<String>,
    pub jvm_options: Vec<String>,
    pub sources: Vec<String>,
}

impl ScriptDirectives {
    /// Parse script directives from Java source code content.
    /// Supports standard JBang comment directives (`//DEPS`, `//JAVA`, `//JAVA_OPTIONS`, etc.)
    /// as well as script frontmatter blocks.
    pub fn parse(content: &str) -> Self {
        let mut directives = Self::default();

        for line in content.lines() {
            let trimmed = line.trim();

            // Stop directive parsing once we encounter class, record, interface, or main method
            if trimmed.starts_with("public class")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("public interface")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("public record")
                || trimmed.starts_with("record ")
                || trimmed.starts_with("public enum")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("void main(")
                || trimmed.starts_with("public static void main(")
            {
                break;
            }

            let directive_body = if trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
            {
                trimmed
                    .trim_start_matches('/')
                    .trim_start_matches('*')
                    .trim()
            } else {
                trimmed
            };

            if let Some(deps_str) = directive_body.strip_prefix("DEPS") {
                let deps_content = deps_str.trim_start_matches('=').trim_start_matches(':');
                for part in deps_content.split(|c: char| c.is_whitespace() || c == ',') {
                    let clean =
                        part.trim_matches(|c| c == '"' || c == '\'' || c == '[' || c == ']');
                    if !clean.is_empty() {
                        directives.dependencies.push(clean.to_string());
                    }
                }
            } else if let Some(opts_str) = directive_body
                .strip_prefix("JAVA_OPTIONS")
                .or_else(|| directive_body.strip_prefix("JVM_ARGS"))
                .or_else(|| directive_body.strip_prefix("JAVAC_OPTIONS"))
            {
                let opts_content = opts_str.trim_start_matches('=').trim_start_matches(':');
                for opt in opts_content.split_whitespace() {
                    let clean = opt.trim().trim_matches(|c| c == '"' || c == '\'');
                    if !clean.is_empty() {
                        directives.jvm_options.push(clean.to_string());
                    }
                }
            } else if let Some(java_str) = directive_body
                .strip_prefix("JAVA ")
                .or_else(|| directive_body.strip_prefix("JAVA="))
                .or_else(|| directive_body.strip_prefix("JAVA:"))
            {
                let j = java_str
                    .trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .trim_end_matches('+');
                if !j.is_empty() {
                    directives.java_version = Some(j.to_string());
                }
            } else if let Some(files_str) = directive_body
                .strip_prefix("FILES")
                .or_else(|| directive_body.strip_prefix("SOURCES"))
            {
                let files_content = files_str.trim_start_matches('=').trim_start_matches(':');
                for f in files_content.split(|c: char| c.is_whitespace() || c == ',') {
                    let clean = f.trim().trim_matches(|c| c == '"' || c == '\'');
                    if !clean.is_empty() {
                        directives.sources.push(clean.to_string());
                    }
                }
            } else if trimmed.starts_with('"') || trimmed.starts_with('\'') {
                // Support dependencies listed within a multiline block
                let clean = trimmed
                    .trim_matches(|c| c == ',' || c == '"' || c == '\'' || c == '[' || c == ']');
                if clean.contains(':') {
                    directives.dependencies.push(clean.to_string());
                }
            }
        }

        directives
    }
}

/// Resolves a dependency coordinate shorthand or missing version.
pub async fn resolve_coordinate(
    coord: &str,
    central: &MavenCentralClient,
) -> Result<(String, String, String)> {
    let expanded = match coord.to_ascii_lowercase().as_str() {
        "gson" => "com.google.code.gson:gson",
        "guava" => "com.google.guava:guava",
        "lombok" => "org.projectlombok:lombok",
        "jackson" => "com.fasterxml.jackson.core:jackson-databind",
        "okhttp" => "com.squareup.okhttp3:okhttp",
        "picocli" => "info.picocli:picocli",
        "slf4j" => "org.slf4j:slf4j-simple",
        "postgres" | "postgresql" => "org.postgresql:postgresql",
        "mysql" => "com.mysql:mysql-connector-j",
        "h2" => "com.h2database:h2",
        "jsoup" => "org.jsoup:jsoup",
        _ => coord,
    };

    let parts: Vec<&str> = expanded.split(':').collect();
    match parts.as_slice() {
        [group, artifact, version] => Ok((group.to_string(), artifact.to_string(), version.to_string())),
        [group, artifact] => {
            let ver = central.resolve_latest_version(group, artifact).await?;
            Ok((group.to_string(), artifact.to_string(), ver))
        }
        _ => Err(DjukeError::Generic(format!(
            "Invalid dependency coordinate '{}'. Format: 'groupId:artifactId:version' or alias (e.g. 'gson')",
            coord
        ))),
    }
}

/// Downloads and caches script dependencies from Maven Central, returning local JAR paths.
pub async fn resolve_script_dependencies(
    deps: &[String],
    cache_dir: &Path,
    quiet: bool,
) -> Result<Vec<PathBuf>> {
    let jars_dir = cache_dir.join("jars");
    fs::create_dir_all(&jars_dir)?;

    let central = MavenCentralClient::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("djuke-cli")
        .build()
        .unwrap_or_default();

    let mut resolved_jars: Vec<PathBuf> = Vec::new();
    let mut visited: HashSet<(String, String)> = HashSet::new();

    for raw_dep in deps {
        let (group, artifact, version) = resolve_coordinate(raw_dep, &central).await?;
        download_jar_and_transitive(
            &group,
            &artifact,
            &version,
            &jars_dir,
            &client,
            &mut visited,
            &mut resolved_jars,
            quiet,
        )
        .await?;
    }

    Ok(resolved_jars)
}

/// Download a JAR and its immediate direct compile/runtime dependencies from its POM.
#[allow(clippy::too_many_arguments)]
async fn download_jar_and_transitive(
    group: &str,
    artifact: &str,
    version: &str,
    jars_dir: &Path,
    client: &reqwest::Client,
    visited: &mut HashSet<(String, String)>,
    resolved_jars: &mut Vec<PathBuf>,
    quiet: bool,
) -> Result<()> {
    let key = (group.to_string(), artifact.to_string());
    if visited.contains(&key) {
        return Ok(());
    }
    visited.insert(key);

    let group_path = group.replace('.', "/");
    let jar_name = format!("{}-{}-{}.jar", group, artifact, version);
    let target_jar = jars_dir.join(&jar_name);

    // Also check standard ~/.m2/repository cache if present
    let m2_jar = dirs_home().map(|h| {
        h.join(".m2")
            .join("repository")
            .join(&group_path)
            .join(artifact)
            .join(version)
            .join(format!("{}-{}.jar", artifact, version))
    });

    let final_path = if let Some(m2) = m2_jar.filter(|p| p.is_file()) {
        m2
    } else if target_jar.is_file() {
        target_jar
    } else {
        let jar_url = format!(
            "{}/{}/{}/{}/{}-{}.jar",
            MAVEN_CENTRAL_REPO_URL, group_path, artifact, version, artifact, version
        );
        if !quiet {
            println!(
                "  {} {} ({})",
                "•".cyan(),
                format!("{}:{}:{}", group, artifact, version).green(),
                "downloading".yellow()
            );
        }

        let resp = client.get(&jar_url).send().await.map_err(|e| {
            DjukeError::Network(format!("Failed to download JAR from {}: {}", jar_url, e))
        })?;

        if !resp.status().is_success() {
            return Err(DjukeError::Network(format!(
                "Failed to fetch {}: HTTP status {}",
                jar_url,
                resp.status()
            )));
        }

        let bytes = resp.bytes().await.map_err(|e| {
            DjukeError::Network(format!("Failed to read JAR bytes for {}: {}", jar_name, e))
        })?;

        fs::write(&target_jar, &bytes)?;
        target_jar
    };

    if !resolved_jars.contains(&final_path) {
        resolved_jars.push(final_path);
    }

    // Attempt to parse POM for direct compile/runtime dependencies
    let pom_url = format!(
        "{}/{}/{}/{}/{}-{}.pom",
        MAVEN_CENTRAL_REPO_URL, group_path, artifact, version, artifact, version
    );

    if let Ok(resp) = client.get(&pom_url).send().await {
        if resp.status().is_success() {
            if let Ok(pom_text) = resp.text().await {
                let direct_deps = parse_pom_dependencies(&pom_text);
                for (dep_group, dep_art, dep_ver) in direct_deps {
                    if !visited.contains(&(dep_group.clone(), dep_art.clone())) {
                        let _ = Box::pin(download_jar_and_transitive(
                            &dep_group,
                            &dep_art,
                            &dep_ver,
                            jars_dir,
                            client,
                            visited,
                            resolved_jars,
                            quiet,
                        ))
                        .await;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Helper to parse dependencies from POM XML
fn parse_pom_dependencies(pom_xml: &str) -> Vec<(String, String, String)> {
    let mut deps = Vec::new();
    let re_dep = regex::Regex::new(r"(?s)<dependency>(.*?)</dependency>").unwrap();
    let re_g = regex::Regex::new(r"<groupId>([^<]+)</groupId>").unwrap();
    let re_a = regex::Regex::new(r"<artifactId>([^<]+)</artifactId>").unwrap();
    let re_v = regex::Regex::new(r"<version>([^<]+)</version>").unwrap();
    let re_scope = regex::Regex::new(r"<scope>([^<]+)</scope>").unwrap();
    let re_opt = regex::Regex::new(r"<optional>([^<]+)</optional>").unwrap();

    for cap in re_dep.captures_iter(pom_xml) {
        let block = &cap[1];

        // Skip test and provided scopes
        if let Some(scope_cap) = re_scope.captures(block) {
            let scope = scope_cap[1].trim();
            if scope == "test" || scope == "provided" || scope == "system" {
                continue;
            }
        }

        // Skip optional dependencies
        if let Some(opt_cap) = re_opt.captures(block) {
            let opt = opt_cap[1].trim();
            if opt.eq_ignore_ascii_case("true") {
                continue;
            }
        }

        let group = re_g.captures(block).map(|c| c[1].trim().to_string());
        let art = re_a.captures(block).map(|c| c[1].trim().to_string());
        let ver = re_v.captures(block).map(|c| c[1].trim().to_string());

        if let (Some(g), Some(a), Some(v)) = (group, art, ver) {
            // Skip unresolved POM properties like ${project.version} or ${...}
            if !v.starts_with('$') && !v.is_empty() {
                deps.push((g, a, v));
            }
        }
    }

    deps
}

fn dirs_home() -> Option<PathBuf> {
    dirs_sys::home_dir()
}

mod dirs_sys {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("HOME").map(PathBuf::from)
        }
    }
}

/// Builds a platform-specific classpath string from resolved JAR paths.
pub fn build_classpath(jars: &[PathBuf]) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    jars.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_script_directives() {
        let script = r#"
//DEPS com.google.code.gson:gson:2.11.0 org.apache.commons:commons-lang3:3.17.0
//JAVA 21
//JAVA_OPTIONS --enable-preview -Xmx512m
//FILES helper/Util.java

import com.google.gson.Gson;

public class Script {
    public static void main(String[] args) {
        System.out.println("Hello");
    }
}
"#;

        let directives = ScriptDirectives::parse(script);
        assert_eq!(
            directives.dependencies,
            vec![
                "com.google.code.gson:gson:2.11.0",
                "org.apache.commons:commons-lang3:3.17.0"
            ]
        );
        assert_eq!(directives.java_version.as_deref(), Some("21"));
        assert_eq!(directives.jvm_options, vec!["--enable-preview", "-Xmx512m"]);
        assert_eq!(directives.sources, vec!["helper/Util.java"]);
    }

    #[test]
    fn test_parse_script_directives_comma_and_slashes() {
        let script = r#"
/// DEPS com.google.code.gson:gson:2.11.0,org.apache.commons:commons-lang3:3.17.0
/// JAVA 21+
/// JVM_ARGS -Xms256m -Xmx1024m
/// SOURCES util/A.java,util/B.java

record Point(int x, int y) {}
void main() {}
"#;

        let directives = ScriptDirectives::parse(script);
        assert_eq!(
            directives.dependencies,
            vec![
                "com.google.code.gson:gson:2.11.0",
                "org.apache.commons:commons-lang3:3.17.0"
            ]
        );
        assert_eq!(directives.java_version.as_deref(), Some("21"));
        assert_eq!(directives.jvm_options, vec!["-Xms256m", "-Xmx1024m"]);
        assert_eq!(directives.sources, vec!["util/A.java", "util/B.java"]);
    }

    #[test]
    fn test_parse_pom_dependencies() {
        let pom = r#"
<project>
    <dependencies>
        <dependency>
            <groupId>org.slf4j</groupId>
            <artifactId>slf4j-api</artifactId>
            <version>2.0.16</version>
        </dependency>
        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>5.11.3</version>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>com.example</groupId>
            <artifactId>optional-lib</artifactId>
            <version>1.0.0</version>
            <optional>true</optional>
        </dependency>
        <dependency>
            <groupId>org.variable</groupId>
            <artifactId>var-lib</artifactId>
            <version>${project.version}</version>
        </dependency>
    </dependencies>
</project>
"#;

        let deps = parse_pom_dependencies(pom);
        assert_eq!(deps.len(), 1);
        assert_eq!(
            deps[0],
            ("org.slf4j".into(), "slf4j-api".into(), "2.0.16".into())
        );
    }

    #[test]
    fn test_build_classpath() {
        let p1 = PathBuf::from("a.jar");
        let p2 = PathBuf::from("b.jar");
        let cp = build_classpath(&[p1, p2]);
        if cfg!(windows) {
            assert_eq!(cp, "a.jar;b.jar");
        } else {
            assert_eq!(cp, "a.jar:b.jar");
        }
    }
}
