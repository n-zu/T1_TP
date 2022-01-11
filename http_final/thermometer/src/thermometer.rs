use crate::{ClientResult, ThermometerObserver};
use config::config::Config;
use mqtt_client::Client;
use packets::publish::Publish;
use packets::qos::QoSLevel;
use packets::PacketResult;
use rand::{thread_rng, Rng};

const MAX_TEMP: f32 = 100.0;
const MIN_TEMP: f32 = 0.0;
const VAR_TEMP: f32 = 10.0;
const SEED_TEMP: Option<f32> = None;

pub struct Thermometer {
    client: Client<ThermometerObserver>,
    config: Config,
}

impl Thermometer {
    pub fn new(client: Client<ThermometerObserver>, config: Config) -> Thermometer {
        Thermometer { client, config }
    }

    pub fn publish(&mut self) -> ClientResult<()> {
        let mut temperature = self.measure_temperature(SEED_TEMP);
        loop {
            temperature = self.measure_temperature(Some(temperature));
            let publish = self.create_publish(temperature)?;
            println!("- - - - - - -\n{:}", publish.payload());
            self.client.publish(publish).expect("Could not publish");
            std::thread::sleep(std::time::Duration::from_millis(self.config.period));
        }
        //Ok(())
    }

    fn measure_temperature(&self, temperature: Option<f32>) -> f32 {
        match temperature {
            None => thread_rng().gen::<f32>() * (MAX_TEMP - MIN_TEMP) + MIN_TEMP,
            Some(t) => {
                let r = t + thread_rng().gen::<f32>() * VAR_TEMP * 2.0 - VAR_TEMP;
                match r {
                    t if t > MAX_TEMP => MAX_TEMP,
                    t if t < MIN_TEMP => MIN_TEMP,
                    _ => r,
                }
            }
        }
    }

    fn create_publish(&self, temperature: f32) -> PacketResult<Publish> {
        Publish::new(
            false,
            QoSLevel::QoSLevel0,
            false,
            &self.config.topic,
            &temperature.to_string(),
            None,
        )
    }
}
