fn main() -> clef_model::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("pass a snapshot directory")?;
    let width = std::env::args().nth(2).ok_or("pass 1024 or 760")?.parse()?;
    clef_ui::gallery::render_routing(output.into(), width)
}
