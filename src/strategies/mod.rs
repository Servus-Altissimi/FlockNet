pub mod static_strategies;
pub mod red;
pub mod blue;
pub mod codel;
pub mod pie;
pub mod fq_codel;
pub mod template;

use crate::network::Packet;
use std::fmt;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Accept,
    Drop,
    Mark,
}

pub trait Strategy: Send + Sync + fmt::Debug {
    fn on_enqueue(&mut self, packet: &Packet, queue_len: usize) -> Action;
    fn on_dequeue(&mut self, queue_len: usize);
    fn update(&mut self, queue_len: usize, avg_sojourn_ms: f64);
    fn name(&self) -> &str;
    fn reset(&mut self);
    fn clone_box(&self) -> Box<dyn Strategy>;
}

pub struct StrategyRegistry {
    strategies: HashMap<String, Box<dyn Fn(usize) -> Box<dyn Strategy> + Send + Sync>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            strategies: HashMap::new(),
        };
        registry.register_builtin();
        registry
    }
    
    fn register_builtin(&mut self) {
        self.register("drop-tail", |size| Box::new(static_strategies::DropTail::new(size)));
        self.register("droptail", |size| Box::new(static_strategies::DropTail::new(size)));
        self.register("fifo", |size| Box::new(static_strategies::Fifo::new(size)));
        self.register("red", |size| Box::new(red::Red::new(size)));
        self.register("adaptive-red", |size| Box::new(red::AdaptiveRed::new(size)));
        self.register("ared", |size| Box::new(red::AdaptiveRed::new(size)));
        self.register("blue", |size| Box::new(blue::Blue::new(size)));
        self.register("codel", |_| Box::new(codel::CoDel::new()));
        self.register("pie", |_| Box::new(pie::Pie::new()));
        self.register("fq-codel", |size| Box::new(fq_codel::FqCoDel::new(size)));
        self.register("fqcodel", |size| Box::new(fq_codel::FqCoDel::new(size)));
    }
    
    pub fn register<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(usize) -> Box<dyn Strategy> + Send + Sync + 'static,
    {
        self.strategies.insert(name.to_lowercase(), Box::new(factory));
    }
    
    pub fn create(&self, name: &str, buffer_size: usize) -> Option<Box<dyn Strategy>> {
        self.strategies
            .get(&name.to_lowercase())
            .map(|factory| factory(buffer_size))
    }
    
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.strategies.keys().cloned().collect();
        names.sort();
        names
    }
    
    pub fn global() -> &'static StrategyRegistry {
        use std::sync::OnceLock;
        static REGISTRY: OnceLock<StrategyRegistry> = OnceLock::new();
        REGISTRY.get_or_init(StrategyRegistry::new)
    }
}

pub struct StrategyBuilder {
    name: String,
    buffer_size: usize,
    params: HashMap<String, f64>,
}

impl StrategyBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            buffer_size: 1024,
            params: HashMap::new(),
        }
    }
    
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
    
    pub fn param(mut self, key: impl Into<String>, value: f64) -> Self {
        self.params.insert(key.into(), value);
        self
    }
    
    pub fn build(self) -> Option<Box<dyn Strategy>> {
        StrategyRegistry::global().create(&self.name, self.buffer_size)
    }
}