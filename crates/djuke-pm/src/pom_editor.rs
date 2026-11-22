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

    let new_content = if let Some(pos) = content.find("</dependencies>") {
        let (head, tail) = content.split_at(pos);
        format!("{}{}{}", head, dep_xml, tail)
    } else if let Some(pos) = content.rfind("</project>") {
        let (head, tail) = content.split_at(pos);
        format!(
            "{}\n    <dependencies>\n{}    </dependencies>\n{}",
            head.trim_end(),
            dep_xml,
            tail
        )
    } else {
        return Err(DjukeError::Generic(format!(
            "Invalid pom.xml at '{}': missing </project> closing tag",
            pom_path.display()
        )));
    };

    fs::write(pom_path, new_content)?;
    Ok(())
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
