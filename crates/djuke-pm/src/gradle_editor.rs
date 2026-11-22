use std::fs;
use std::path::Path;

use djuke_core::Result;

use crate::dependency::DependencySpec;

pub fn add_gradle_dependency(build_file: &Path, dep: &DependencySpec) -> Result<()> {
    let content = fs::read_to_string(build_file)?;
    let is_kts = build_file.to_string_lossy().ends_with(".kts");

    // Check if already present
    let coord_partial = format!("{}:{}", dep.group_id, dep.artifact_id);
    if content.contains(&coord_partial) {
        return Ok(());
    }

    let scope_name = dep.scope.as_gradle_scope();
    let dep_line = match &dep.version {
        Some(v) => {
            if is_kts {
                format!(
                    "    {}(\"{}:{}:{}\")",
                    scope_name, dep.group_id, dep.artifact_id, v
                )
            } else {
                format!(
                    "    {} '{}:{}:{}'",
                    scope_name, dep.group_id, dep.artifact_id, v
                )
            }
        }
        None => {
            if is_kts {
                format!(
                    "    {}(\"{}:{}\")",
                    scope_name, dep.group_id, dep.artifact_id
                )
            } else {
                format!("    {} '{}:{}'", scope_name, dep.group_id, dep.artifact_id)
            }
        }
    };

    let new_content = if let Some(pos) = content.find("dependencies {") {
        let insert_point = pos + "dependencies {".len();
        let (head, tail) = content.split_at(insert_point);
        format!("{}\n{}{}", head, dep_line, tail)
    } else {
        // Append dependencies block to end of file
        format!(
            "{}\n\ndependencies {{\n{}\n}}\n",
            content.trim_end(),
            dep_line
        )
    };

    fs::write(build_file, new_content)?;
    Ok(())
}

pub fn remove_gradle_dependency(build_file: &Path, group: &str, artifact: &str) -> Result<()> {
    let content = fs::read_to_string(build_file)?;
    let coord = format!("{}:{}", group, artifact);
    let mut new_lines = Vec::new();

    for line in content.lines() {
        if line.contains(&coord) {
            // Skip this dependency line
            continue;
        }
        new_lines.push(line);
    }

    fs::write(build_file, new_lines.join("\n") + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependency::DependencyScope;
    use tempfile::NamedTempFile;

    #[test]
    fn test_add_and_remove_gradle_dependency() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "plugins { java }\n\ndependencies {\n    testImplementation(\"junit:junit:4.13.2\")\n}\n").unwrap();

        let dep = DependencySpec {
            group_id: "org.postgresql".into(),
            artifact_id: "postgresql".into(),
            version: Some("42.7.4".into()),
            scope: DependencyScope::Implementation,
        };

        add_gradle_dependency(file.path(), &dep).unwrap();
        let content = fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("org.postgresql:postgresql:42.7.4"));

        remove_gradle_dependency(file.path(), "org.postgresql", "postgresql").unwrap();
        let updated = fs::read_to_string(file.path()).unwrap();
        assert!(!updated.contains("org.postgresql:postgresql"));
    }
}
