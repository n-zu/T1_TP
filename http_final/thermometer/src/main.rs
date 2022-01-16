use observer::ThermometerObserver;

use crate::thermometer::Thermometer;
use std::error::Error;

mod observer;
mod setup;
mod thermometer;

type ClientResult<T> = Result<T, Box<dyn Error>>;

fn main() {
    setup::init();
}
