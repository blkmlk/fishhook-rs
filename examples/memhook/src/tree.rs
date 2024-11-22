use crate::backtrace::TraceInfo;
use std::collections::HashMap;
use std::hash::Hash;

pub struct Tree {
    root: Node,
    pointers: HashMap<usize, usize>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            root: Node::default(),
            pointers: HashMap::new(),
        }
    }

    pub fn on_malloc(&mut self, tracer: impl Iterator<Item = TraceInfo>, size: usize, ptr: usize) {
        self.pointers.insert(ptr, size);
        self.root.push_alloc(tracer, size);
    }

    pub fn on_calloc(
        &mut self,
        tracer: impl Iterator<Item = TraceInfo>,
        size: usize,
        blk_size: usize,
        ptr: usize,
    ) {
        self.pointers.insert(ptr, size * blk_size);
        self.root.push_alloc(tracer, size * blk_size);
    }

    pub fn on_realloc(&mut self, tracer: impl Iterator<Item = TraceInfo>, size: usize, ptr: usize) {
        let old_size = self.pointers.entry(ptr).or_insert(size);

        if *old_size != size {
            return;
        }

        if *old_size > size {
            self.root.push_alloc(tracer, *old_size - size);
        } else {
            self.root.push_free(tracer, size - *old_size);
        }
    }

    pub fn on_free(&mut self, tracer: impl Iterator<Item = TraceInfo>, ptr: usize) {
        if let Some(size) = self.pointers.remove(&ptr) {
            self.root.push_free(tracer, size);
        }
    }

    pub fn fg_values(&self) -> Vec<String> {
        let mut values = vec![];

        self.root.fg_values(&mut values, "", "");

        values
    }
}

#[derive(Default)]
pub struct Node {
    pub info: TraceInfo,
    pub stats: NodeStats,
    pub children: HashMap<String, Node>,
}

impl Node {
    pub fn push_alloc(&mut self, tracer: impl Iterator<Item = TraceInfo>, size: usize) {
        let f = |s: &mut NodeStats| {
            s.allocated += size;
            s.total_allocated += size;
        };

        self.push_and_modify(tracer, f);
    }

    pub fn push_free(&mut self, tracer: impl Iterator<Item = TraceInfo>, size: usize) {
        let f = |s: &mut NodeStats| {
            s.allocated -= size;
            s.total_freed += size;
        };

        self.push_and_modify(tracer, f);
    }

    pub fn fg_values(&self, v: &mut Vec<String>, parent: &str, values: &str) {
        let parent = if parent.is_empty() {
            self.info.function_name.clone()
        } else {
            format!("{};{}", parent, self.info.function_name)
        };

        v.push(format!("{} {}", parent, self.stats.total_allocated));

        for (_, c) in self.children.iter() {
            c.fg_values(v, &parent, &values);
        }
    }

    fn push_and_modify(
        &mut self,
        mut tracer: impl Iterator<Item = TraceInfo>,
        f: impl Fn(&mut NodeStats),
    ) {
        let Some(next) = tracer.next() else {
            f(&mut self.stats);
            return;
        };

        let child = self.children.entry(next.function_name.clone()).or_default();
        child.info = next;

        child.push_and_modify(tracer, f)
    }
}

#[derive(Default)]
pub struct NodeStats {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub num_allocations: usize,
    pub allocated: usize,
}
