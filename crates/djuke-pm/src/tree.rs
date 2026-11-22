use colored::Colorize;

use crate::dependency::{DependencyScope, DependencySpec};

pub fn render_dependency_tree(project_name: &str, dependencies: &[DependencySpec]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", project_name.bold()));

    if dependencies.is_empty() {
        out.push_str("└── (no declared dependencies)\n");
        return out;
    }

    let scopes = [
        (
            DependencyScope::Implementation,
            "dependencies (implementation)",
        ),
        (DependencyScope::CompileOnly, "compileOnly"),
        (DependencyScope::RuntimeOnly, "runtimeOnly"),
        (DependencyScope::Test, "test dependencies"),
    ];

    let mut present_groups = Vec::new();
    for (scope, label) in &scopes {
        let matching: Vec<&DependencySpec> =
            dependencies.iter().filter(|d| d.scope == *scope).collect();
        if !matching.is_empty() {
            present_groups.push((*scope, *label, matching));
        }
    }

    let total_groups = present_groups.len();
    for (g_idx, (_scope, label, deps)) in present_groups.iter().enumerate() {
        let is_last_group = g_idx == total_groups - 1;
        let group_prefix = if is_last_group {
            "└── "
        } else {
            "├── "
        };
        let child_indent = if is_last_group { "    " } else { "│   " };

        out.push_str(&format!("{}{}\n", group_prefix, label.cyan().bold()));

        for (d_idx, dep) in deps.iter().enumerate() {
            let is_last_dep = d_idx == deps.len() - 1;
            let dep_prefix = if is_last_dep {
                "└── "
            } else {
                "├── "
            };

            let ver_str = dep
                .version
                .as_deref()
                .map(|v| format!(":{}", v.green()))
                .unwrap_or_else(|| "".to_string());

            out.push_str(&format!(
                "{}{}{}{}{}{}\n",
                child_indent,
                dep_prefix,
                dep.group_id.dimmed(),
                ":".dimmed(),
                dep.artifact_id.bold(),
                ver_str
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_dependency_tree() {
        let deps = vec![
            DependencySpec {
                group_id: "com.google.guava".to_string(),
                artifact_id: "guava".to_string(),
                version: Some("33.0.0-jre".to_string()),
                scope: DependencyScope::Implementation,
            },
            DependencySpec {
                group_id: "org.junit.jupiter".to_string(),
                artifact_id: "junit-jupiter".to_string(),
                version: Some("5.10.0".to_string()),
                scope: DependencyScope::Test,
            },
        ];

        let tree = render_dependency_tree("my-app", &deps);
        assert!(tree.contains("my-app"));
        assert!(tree.contains("guava"));
        assert!(tree.contains("junit-jupiter"));
    }
}
