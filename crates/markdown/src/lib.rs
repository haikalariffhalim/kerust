// markdown/src/lib.rs
use anyhow::Result;
use specs::Spec;

/// Render a Spec into Markdown. This crate depends only on core types.
pub fn render_to_markdown(specs: &Spec) -> Result<String> {
    let mut md = String::new();
    md.push_str(&format!(
        "# {} {}\n\n",
        specs.brand.clone().unwrap_or_default(),
        specs.model
    ));
    md.push_str("| Field | Value |\n|---|---|\n");
    md.push_str(&format!(
        "| Display | {} |\n",
        specs.display.clone().unwrap_or("N/A".to_string())
    ));
    md.push_str(&format!(
        "| Memory | {} |\n",
        specs.memory.clone().unwrap_or("N/A".to_string())
    ));
    md.push_str(&format!(
        "| Storage | {} |\n",
        specs.storage.clone().unwrap_or("N/A".to_string())
    ));
    if let Some(lens) = &specs.cameras {
        md.push_str(&format!(
            "| Main camera | {} |\n",
            lens.main.clone().unwrap_or("N/A".to_string())
        ));
        md.push_str(&format!(
            "| Telephoto | {} |\n",
            lens.telephoto.clone().unwrap_or("N/A".to_string())
        ));
    } else {
        md.push_str("| Cameras | N/A |\n");
    }
    Ok(md)
}
