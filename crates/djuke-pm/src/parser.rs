use std::collections::HashMap;
use std::fs;
use std::path::Path;

use regex::Regex;

use djuke_core::{DjukeError, Result};

use crate::dependency::{DependencyScope, DependencySpec};

pub fn extract_project_dependencies(project_root: &Path) -> Result<Vec<DependencySpec>> {
    let pom = project_root.join("pom.xml");
    let gradle_kts = project_root.join("build.gradle.kts");
    let gradle_groovy = project_root.join("build.gradle");

    if pom.is_file() {
        let content = fs::read_to_string(&pom)?;
        Ok(extract_pom_dependencies(&content))
    } else if gradle_kts.is_file() {
        let content = fs::read_to_string(&gradle_kts)?;
        Ok(extract_gradle_dependencies(&content))
    } else if gradle_groovy.is_file() {
        let content = fs::read_to_string(&gradle_groovy)?;
        Ok(extract_gradle_dependencies(&content))
    } else {
        Err(DjukeError::NoBuildToolFound(format!(
            "No build file (pom.xml, build.gradle.kts, or build.gradle) found in '{}'",
            project_root.display()
        )))
    }
}

pub fn extract_pom_dependencies(content: &str) -> Vec<DependencySpec> {
    let mut deps = Vec::new();

    // First extract properties for placeholder resolution
    let prop_re = Regex::new(r"<([a-zA-Z0-9._-]+)>([^<]+)</[a-zA-Z0-9._-]+>").unwrap();
    let mut properties = HashMap::new();
    if let Some(prop_start) = content.find("<properties>") {
        if let Some(prop_end) = content.find("</properties>") {
            let prop_block = &content[prop_start..prop_end];
            for cap in prop_re.captures_iter(prop_block) {
                properties.insert(cap[1].to_string(), cap[2].trim().to_string());
            }
        }
    }

    let dep_block_re = Regex::new(r"(?s)<dependency>(.*?)</dependency>").unwrap();
    let group_re = Regex::new(r"<groupId>\s*([^<\s]+)\s*</groupId>").unwrap();
    let artifact_re = Regex::new(r"<artifactId>\s*([^<\s]+)\s*</artifactId>").unwrap();
    let version_re = Regex::new(r"<version>\s*([^<\s]+)\s*</version>").unwrap();
    let scope_re = Regex::new(r"<scope>\s*([^<\s]+)\s*</scope>").unwrap();

    for block_cap in dep_block_re.captures_iter(content) {
        let block = &block_cap[1];
        let group_id = group_re.captures(block).map(|c| c[1].to_string());
        let artifact_id = artifact_re.captures(block).map(|c| c[1].to_string());

        if let (Some(group_id), Some(artifact_id)) = (group_id, artifact_id) {
            let mut version = version_re.captures(block).map(|c| c[1].to_string());
            if let Some(v) = &version {
                if v.starts_with("${") && v.ends_with('}') {
                    let prop_key = &v[2..v.len() - 1];
                    if let Some(resolved) = properties.get(prop_key) {
                        version = Some(resolved.clone());
                    }
                }
            }

            let scope = if let Some(s_cap) = scope_re.captures(block) {
                match s_cap[1].to_ascii_lowercase().as_str() {
                    "test" => DependencyScope::Test,
                    "provided" => DependencyScope::CompileOnly,
                    "runtime" => DependencyScope::RuntimeOnly,
                    _ => DependencyScope::Implementation,
                }
            } else {
                DependencyScope::Implementation
            };

            deps.push(DependencySpec {
                group_id,
                artifact_id,
                version,
                scope,
            });
        }
    }

    deps
}

pub fn extract_gradle_dependencies(content: &str) -> Vec<DependencySpec> {
    let mut deps = Vec::new();

    // Matches e.g.:
    // implementation("com.example:foo:1.0.0")
    // testImplementation("com.example:foo")
    // implementation 'com.example:foo:1.0.0'
    // implementation platform("com.example:foo:1.0.0")
    let re = Regex::new(
        r#"(?m)^\s*([a-zA-Z]+)\s*\(?(?:platform\(|enforcedPlatform\()?['"]([^'"\s]+:[^'"\s]+)['"]\)?\)?\s*$"#,
    )
    .unwrap();

    for cap in re.captures_iter(content) {
        let scope_str = &cap[1];
        let raw_coord = &cap[2];

        let scope = match scope_str {
            "testImplementation" | "testRuntimeOnly" | "testCompileOnly" => DependencyScope::Test,
            "compileOnly" => DependencyScope::CompileOnly,
            "runtimeOnly" => DependencyScope::RuntimeOnly,
            _ => DependencyScope::Implementation,
        };

        let parts: Vec<&str> = raw_coord.split(':').collect();
        if parts.len() == 2 {
            deps.push(DependencySpec {
                group_id: parts[0].to_string(),
                artifact_id: parts[1].to_string(),
                version: None,
                scope,
            });
        } else if parts.len() >= 3 {
            deps.push(DependencySpec {
                group_id: parts[0].to_string(),
                artifact_id: parts[1].to_string(),
                version: Some(parts[2].to_string()),
                scope,
            });
        }
    }

    deps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pom_dependencies() {
        let xml = r#"
        <project>
            <properties>
                <pg.version>42.7.2</pg.version>
            </properties>
            <dependencies>
                <dependency>
                    <groupId>org.postgresql</groupId>
                    <artifactId>postgresql</artifactId>
                    <version>${pg.version}</version>
                    <scope>runtime</scope>
                </dependency>
                <dependency>
                    <groupId>org.junit.jupiter</groupId>
                    <artifactId>junit-jupiter</artifactId>
                    <scope>test</scope>
                </dependency>
            </dependencies>
        </project>
        "#;

        let deps = extract_pom_dependencies(xml);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].group_id, "org.postgresql");
        assert_eq!(deps[0].version.as_deref(), Some("42.7.2"));
        assert_eq!(deps[0].scope, DependencyScope::RuntimeOnly);
        assert_eq!(deps[1].artifact_id, "junit-jupiter");
        assert_eq!(deps[1].scope, DependencyScope::Test);
    }

    #[test]
    fn test_extract_gradle_dependencies() {
        let kts = r#"
        dependencies {
            implementation("com.google.guava:guava:33.0.0-jre")
            testImplementation("org.junit.jupiter:junit-jupiter:5.10.0")
            compileOnly("org.projectlombok:lombok:1.18.30")
        }
        "#;

        let deps = extract_gradle_dependencies(kts);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].group_id, "com.google.guava");
        assert_eq!(deps[0].version.as_deref(), Some("33.0.0-jre"));
        assert_eq!(deps[0].scope, DependencyScope::Implementation);
        assert_eq!(deps[1].scope, DependencyScope::Test);
        assert_eq!(deps[2].scope, DependencyScope::CompileOnly);
    }
}
