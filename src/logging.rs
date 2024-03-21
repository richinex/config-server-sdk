
use slog::{Drain, Logger, o};
use slog_async::Async;
use slog_json::Json;
use slog_term::{FullFormat, TermDecorator};

pub fn configure_logging() -> Logger {
    let decorator = TermDecorator::new().build();
    let console_drain = FullFormat::new(decorator).build().fuse();
    let console_drain = Async::new(console_drain).build().fuse();

    let json_drain = Json::new(std::io::stdout())
        .add_default_keys()
        .build().fuse();
    let json_drain = Async::new(json_drain).build().fuse();

    Logger::root(slog::Duplicate::new(console_drain, json_drain).fuse(), o!())
}
