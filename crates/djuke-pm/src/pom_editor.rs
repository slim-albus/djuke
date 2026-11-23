use std::fs;
use std::path::Path;

use djuke_core::{DjukeError, Result};

use crate::dependency::DependencySpec;

pub fn add_maven_dependency(pom_path: &Path, dep: &DependencySpec) -> Result<()> {
    let content = fs::read_to_string(pom_path)?;

    // Check if already present
    if content.contains(&format!("<groupId>{}</groupId>", dep.group_id))
        && content.contains(&format!("<artifactId>{}</artifactId>", dep.artifact_id))
    {
        return Ok(()); // Already present
    }

    let version_tag = if let Some(v) = &dep.version {
        format!("\n            <version>{}</version>", v)
    } else {
        "".to_string()
    };

    let scope_tag = if let Some(s) = dep.scope.as_maven_scope() {
        format!("\n            <scope>{}</scope>", s)
    } else {
        "".to_string()
    };

    let dep_xml = format!(
        r#"        <dependency>
            <groupId>{}</groupId>
            <artifactId>{}</artifactId>{}{}
        </dependency>
"#,
        dep.group_id, dep.artifact_id, version_tag, scope_tag
    );

    let new_content = insert_dependency_into_pom(&content, &dep_xml, pom_path)?;

    fs::write(pom_path, new_content)?;
    Ok(())
}

fn insert_dependency_into_pom(content: &str, dep_xml: &str, pom_path: &Path) -> Result<String> {
    // Find dependencyManagement spans so we do not insert direct dependencies into management
    let mut dep_mgmt_spans = Vec::new();
    let mut cursor = 0;
    while let Some(start) = content[cursor..].find("<dependencyManagement>") {
        let abs_start = cursor + start;
        if let Some(end) = content[abs_start..].find("</dependencyManagement>") {
            let abs_end = abs_start + end + "</dependencyManagement>".len();
            dep_mgmt_spans.push((abs_start, abs_end));
            cursor = abs_end;
        } else {
            break;
        }
    }

    let is_inside_mgmt =
        |pos: usize| -> bool { dep_mgmt_spans.iter().any(|(s, e)| pos >= *s && pos < *e) };

    // Find a </dependencies> tag that is NOT inside <dependencyManagement>
    let mut target_close_tag = None;
    let mut search_idx = 0;
    while let Some(pos) = content[search_idx..].find("</dependencies>") {
        let abs_pos = search_idx + pos;
        if !is_inside_mgmt(abs_pos) {
            target_close_tag = Some(abs_pos);
            break;
        }
        search_idx = abs_pos + "</dependencies>".len();
    }

    if let Some(pos) = target_close_tag {
        let (head, tail) = content.split_at(pos);
        Ok(format!("{}{}{}", head, dep_xml, tail))
    } else if let Some((_, last_mgmt_end)) = dep_mgmt_spans.last() {
        // If POM has <dependencyManagement> but no direct <dependencies>, insert direct dependencies block right after management
        let (head, tail) = content.split_at(*last_mgmt_end);
        Ok(format!(
            "{}\n\n    <dependencies>\n{}    </dependencies>{}",
            head.trim_end(),
            dep_xml,
            tail
        ))
    } else if let Some(pos) = content.rfind("</project>") {
        let (head, tail) = content.split_at(pos);
        Ok(format!(
            "{}\n    <dependencies>\n{}    </dependencies>\n{}",
            head.trim_end(),
            dep_xml,
            tail
        ))
    } else {
        Err(DjukeError::Generic(format!(
            "Invalid pom.xml at '{}': missing </project> closing tag",
            pom_path.display()
        )))
    }
}

pub fn remove_maven_dependency(pom_path: &Path, group: &str, artifact: &str) -> Result<()> {
    let content = fs::read_to_string(pom_path)?;
    let mut lines = Vec::new();
    let mut in_target_dep = false;
    let mut buffer = Vec::new();

    for line in content.lines() {
        if line.contains("<dependency>") {
            in_target_dep = true;
            buffer.clear();
            buffer.push(line);
            continue;
        }

        if in_target_dep {
            buffer.push(line);
            if line.contains("</dependency>") {
                in_target_dep = false;
                let full_block = buffer.join("\n");
                let matches_group = full_block.contains(&format!("<groupId>{}</groupId>", group));
                let matches_artifact =
                    full_block.contains(&format!("<artifactId>{}</artifactId>", artifact));

                if matches_group && matches_artifact {
                    // Skip adding this block (deleting it)
                    buffer.clear();
                    continue;
                } else {
                    lines.append(&mut buffer);
                }
            }
            continue;
        }

        lines.push(line);
    }

    fs::write(pom_path, lines.join("\n") + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_add_dependency_standard_pom() {
        let dir = tempdir().unwrap();
        let pom = dir.path().join("pom.xml");
        let initial = r#"<project>
    <dependencies>
        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>5.10.0</version>
        </dependency>
    </dependencies>
</project>"#;
        fs::write(&pom, initial).unwrap();

        let dep = DependencySpec {
            group_id: "com.google.guava".into(),
            artifact_id: "guava".into(),
            version: Some("33.0.0-jre".into()),
            scope: crate::dependency::DependencyScope::Implementation,
        };

        add_maven_dependency(&pom, &dep).unwrap();
        let updated = fs::read_to_string(&pom).unwrap();
        assert!(updated.contains("<artifactId>guava</artifactId>"));
        assert!(updated.contains("<version>33.0.0-jre</version>"));
    }

    #[test]
    fn test_add_dependency_avoids_dependency_management() {
        let dir = tempdir().unwrap();
        let pom = dir.path().join("pom.xml");
        let initial = r#"<project>
    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-dependencies</artifactId>
                <version>3.2.0</version>
                <type>pom</type>
                <scope>import</scope>
            </dependency>
        </dependencies>
    </dependencyManagement>

    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;
        fs::write(&pom, initial).unwrap();

        let dep = DependencySpec {
            group_id: "org.postgresql".into(),
            artifact_id: "postgresql".into(),
            version: Some("42.7.2".into()),
            scope: crate::dependency::DependencyScope::Implementation,
        };

        add_maven_dependency(&pom, &dep).unwrap();
        let updated = fs::read_to_string(&pom).unwrap();

        // Ensure postgresql is placed in the direct <dependencies> section, NOT inside <dependencyManagement>
        let mgmt_end = updated.find("</dependencyManagement>").unwrap();
        let postgres_pos = updated.find("<artifactId>postgresql</artifactId>").unwrap();
        assert!(
            postgres_pos > mgmt_end,
            "Dependency should be placed AFTER </dependencyManagement>"
        );
    }
}
