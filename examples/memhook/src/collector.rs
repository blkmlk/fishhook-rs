use crate::backtrace::MyBacktrace;
use crate::tree::Tree;
use inferno::flamegraph;

pub struct Collector {
    allocations: Tree,
}

impl Collector {
    pub fn new() -> Self {
        Self {
            allocations: Tree::new(),
        }
    }

    pub fn on_malloc(&mut self, size: usize, ptr: usize) {
        let bt = MyBacktrace::new();

        self.allocations.on_malloc(bt, size, ptr)
    }

    pub fn on_calloc(&mut self, size: usize, blk_size: usize, ptr: usize) {
        let bt = MyBacktrace::new();

        self.allocations.on_calloc(bt, size, blk_size, ptr)
    }

    pub fn on_realloc(&mut self, size: usize, ptr: usize) {
        let bt = MyBacktrace::new();

        self.allocations.on_realloc(bt, size, ptr)
    }

    pub fn on_free(&mut self, ptr: usize) {
        let bt = MyBacktrace::new();

        self.allocations.on_free(bt, ptr)
    }

    pub fn save_flamegraph(&self, filename: &str) {
        let mut lines = vec![];

        self.allocations.traverse(|node| {
            let fields = node
                .children
                .values()
                .map(|child| child.info.function_name.clone())
                .collect::<Vec<_>>()
                .join(";");

            let values = node
                .children
                .values()
                .map(|child| child.stats.total_allocated.to_string())
                .collect::<Vec<_>>()
                .join(";");

            lines.push(format!("{} {}\n", fields, values));
        });

        let file = std::fs::File::create(filename).unwrap();
        let mut opts = flamegraph::Options::default();

        flamegraph::from_lines(&mut opts, lines.iter().map(|s| s.as_str()), file)
            .expect("failed to save a file");
    }
}
