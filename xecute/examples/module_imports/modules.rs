//! NestJS-style imports/exports: ExampleModule can inject GreetingModule's export.

use super::controller::ExampleController;
use xecute::{Injectable, module};

#[derive(Injectable)]
pub struct GreetingService {}

impl GreetingService {
    pub fn greet(&self, name: &str) -> String {
        format!("Hello, {name}!")
    }
}

#[module(providers = [GreetingService], exports = [GreetingService])]
pub struct GreetingModule;

// Imported services belong to their original module; do not register them twice.
#[module(
    imports = [GreetingModule],
    providers = [ExampleController],
    controllers = [ExampleController],
)]
pub struct ExampleModule;
