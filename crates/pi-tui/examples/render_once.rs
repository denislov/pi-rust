use pi_tui::{ProcessTerminal, Text, Tui};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = ProcessTerminal::new();
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("pi-tui Rust renderer PoC")));
    tui.render_once()?;
    Ok(())
}
