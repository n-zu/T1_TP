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

/// Represents a Thermometer using a MQTT Client that sends its measures
/// to a valid broker. The information is sent with a certain frequency (period)
pub struct Thermometer {
    client: Client<ThermometerObserver>,
    config: Config,
}

impl Thermometer {
    /// Returns a new Thermometer
    pub fn new(client: Client<ThermometerObserver>, config: Config) -> Thermometer {
        Thermometer { client, config }
    }

    /// Publish the measured temperature to the MQTT Broker
    pub fn publish(&mut self) -> ClientResult<()> {
        let mut temperature = self.measure_temperature(SEED_TEMP);
        loop {
            temperature = self.measure_temperature(Some(temperature));
            let publish = self.create_publish(temperature)?;
            println!("- - - - - - -\n{:}", publish.payload());
            self.client.publish(publish).expect("Could not publish");
            std::thread::sleep(std::time::Duration::from_millis(self.config.period));
        }
    }

    /// Algorithm that generates new temperatures based on the given temperature
    #[doc(hidden)]
    fn measure_temperature(&self, old_temperature: Option<f32>) -> f32 {
        match old_temperature {
            None => thread_rng().gen::<f32>() * (MAX_TEMP - MIN_TEMP) + MIN_TEMP,
            Some(old_temp) => {
                let new_temperature =
                    old_temp + thread_rng().gen::<f32>() * VAR_TEMP * 2.0 - VAR_TEMP;
                match new_temperature {
                    t if t > MAX_TEMP => MAX_TEMP,
                    t if t < MIN_TEMP => MIN_TEMP,
                    _ => new_temperature,
                }
            }
        }
    }

    /// Returns a new PUBLISH packet with the given temperature as payload
    #[doc(hidden)]
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
