use std::fs;
use std::path::Path;

use djuke_core::{DjukeError, JdkDistribution, Result};

use crate::model::{BuildToolChoice, NewProjectOptions};

pub const SPRING_INITIALIZR_URL: &str = "https://start.spring.io";

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

        Ok(())
    }
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
