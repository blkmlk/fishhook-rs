use backtrace::Backtrace;

pub struct TraceInfo {
    pub frame_address: usize,
    pub function_name: String,
    pub location: String,
}

pub struct MyBacktrace {
    traces: Vec<TraceInfo>,
}

impl Iterator for MyBacktrace {
    type Item = TraceInfo;

    fn next(&mut self) -> Option<Self::Item> {
        self.traces.pop()
    }
}

impl MyBacktrace {
    pub fn new() -> Self {
        let bt = Backtrace::new();

        let mut traces = Vec::<TraceInfo>::new();

        for frame in bt.frames() {
            for symbol in frame.symbols() {
                if !frame.ip().is_null() {
                    let name = symbol
                        .name()
                        .map_or("<unknown>".to_string(), |name| name.to_string());
                    if let (Some(filename), Some(no)) = (symbol.filename(), symbol.lineno()) {
                        traces.push(TraceInfo {
                            frame_address: frame.ip() as usize,
                            function_name: name,
                            location: format!("{}:{}", filename.display(), no),
                        });
                    }
                }
            }
        }

        Self { traces }
    }
}
