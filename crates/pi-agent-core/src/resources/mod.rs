pub mod frontmatter;
pub mod prompt_templates;
pub mod skills;
pub mod system_prompt;

pub use frontmatter::parse_frontmatter;
pub use prompt_templates::load_prompt_templates;
pub use skills::load_skills;
pub use system_prompt::{
    format_prompt_template_invocation, format_skill_invocation, format_skills_for_system_prompt,
};
