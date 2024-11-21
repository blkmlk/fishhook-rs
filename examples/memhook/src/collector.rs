use crate::backtrace::MyBacktrace;
use crate::tree::Tree;

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
}
