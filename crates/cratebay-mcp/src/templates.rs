//! Sandbox template definitions.
//!
//! Re-exports from `cratebay-core::sandbox` — the canonical source.

#[allow(unused_imports)]
pub use cratebay_core::sandbox::{builtin_templates, find_template, SandboxTemplate};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_templates_count() {
        assert_eq!(builtin_templates().len(), 4);
    }

    #[test]
    fn test_builtin_template_ids() {
        let templates = builtin_templates();
        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"node-dev"));
        assert!(ids.contains(&"python-dev"));
        assert!(ids.contains(&"rust-dev"));
        assert!(ids.contains(&"ubuntu-base"));
    }

    #[test]
    fn test_template_images_are_set() {
        let templates = builtin_templates();
        for template in &templates {
            assert!(
                !template.image.is_empty(),
                "Template '{}' has empty image",
                template.id
            );
        }
    }

    #[test]
    fn test_template_default_resources() {
        let templates = builtin_templates();
        for template in &templates {
            assert!(
                template.default_cpu_cores >= 1,
                "Template '{}' has 0 cpu_cores",
                template.id
            );
            assert!(
                template.default_memory_mb >= 256,
                "Template '{}' has memory_mb < 256",
                template.id
            );
        }
    }

    #[test]
    fn test_template_default_command() {
        let templates = builtin_templates();
        for template in &templates {
            assert_eq!(
                template.default_command, "sleep infinity",
                "Template '{}' has unexpected default command",
                template.id
            );
        }
    }

    #[test]
    fn test_template_tags_not_empty() {
        let templates = builtin_templates();
        for template in &templates {
            assert!(
                !template.tags.is_empty(),
                "Template '{}' has no tags",
                template.id
            );
        }
    }

    #[test]
    fn test_node_dev_template_details() {
        let t = find_template("node-dev").expect("node-dev should exist");
        assert_eq!(t.name, "Node.js Development");
        assert_eq!(t.image, "node:20-slim");
        assert_eq!(t.default_cpu_cores, 2);
        assert_eq!(t.default_memory_mb, 2048);
        assert!(t.tags.contains(&"javascript".to_string()));
        assert!(t.tags.contains(&"typescript".to_string()));
    }

    #[test]
    fn test_rust_dev_template_details() {
        let t = find_template("rust-dev").expect("rust-dev should exist");
        assert_eq!(t.name, "Rust Development");
        assert_eq!(t.image, "rust:1-slim-bookworm");
        assert_eq!(t.default_cpu_cores, 4);
        assert_eq!(t.default_memory_mb, 4096);
    }

    #[test]
    fn test_find_template() {
        assert!(find_template("node-dev").is_some());
        assert!(find_template("python-dev").is_some());
        assert!(find_template("rust-dev").is_some());
        assert!(find_template("ubuntu-base").is_some());
        assert!(find_template("nonexistent").is_none());
    }

    #[test]
    fn test_find_template_case_sensitive() {
        assert!(find_template("Node-Dev").is_none());
        assert!(find_template("NODE-DEV").is_none());
    }

    #[test]
    fn test_templates_serializable() {
        let templates = builtin_templates();
        let json = serde_json::to_string(&templates);
        assert!(json.is_ok(), "Templates should serialize to JSON");
        let json_str = json.unwrap();
        assert!(json_str.contains("node-dev"));
        assert!(json_str.contains("python-dev"));
    }
}
