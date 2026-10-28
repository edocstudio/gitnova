use gitnova_core::Application;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: cargo run -p gitnova-core --example inspect -- /path/to/repo")?;
    let temp = tempfile::tempdir()?;
    let app = Application::new(temp.path().join("workspace.json"))?;
    let catalog = app.add_repository(std::path::Path::new(&path))?;
    let workspace = &catalog.workspaces[0];
    let page = app.graph(workspace.selected_id.as_ref().ok_or("No selection")?, None)?;
    println!("{}", serde_json::to_string_pretty(&page)?);
    Ok(())
}
