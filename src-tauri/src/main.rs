fn main() {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => ctx_ppteer_lib::run(),
        [argument] if argument == "--mcp" => {
            if let Err(error) = ctx_ppteer_lib::mcp::run() {
                eprintln!("ctx-ppteer MCP server failed: {error}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Usage: ctx-ppteer [--mcp]");
            std::process::exit(2);
        }
    }
}
