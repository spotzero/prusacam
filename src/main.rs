use rscam;
use serde::{Serialize, Deserialize};
use std::fs::File;
use ureq;
use std::io::Cursor;
use std::io::Write;
use std::time::{Duration, SystemTime};
use std::thread::sleep;
use rppal::gpio::{Gpio, InputPin, OutputPin};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use signal_hook::consts::signal::SIGUSR1;


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Camera {
    name: String,
    device: String,
    token: String,
    fingerprint: String,
    resolutionx: u32,
    resolutiony: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    cameras: Vec<Camera>,
    gpio_switch: Option<u8>,
    gpio_led: Option<u8>,
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Endpoint {
    name: String,
    interval: u64,
    snapshot_url: String,
    info_url: Option<String>,
}

#[derive(Debug)]
pub struct CameraStatus {
    last_run: SystemTime,
    config: Camera,
}

impl CameraStatus {
    fn grab_image(&mut self) -> Vec<u8> {
        print!("Grabbing image from camera {}", self.config.device);
        let camera_res = rscam::Camera::new(self.config.device.as_str());
        if camera_res.is_err() {
            println!("Error opening camera {}: {:?}", self.config.device, camera_res.err().unwrap());
            return vec![];
        }
        let mut camera = camera_res.unwrap();

        camera.start(&rscam::Config {
            interval: (1, 30),      // 30 fps.
            resolution: (self.config.resolutionx, self.config.resolutiony),
            format: b"MJPG",
            ..Default::default()
        }).unwrap();
        let frame = camera.capture().unwrap();
        let mut buf: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        buf.write_all(&frame).unwrap();
        self.last_run = SystemTime::now();
        buf.into_inner()
    }
}

#[derive(Debug)]
struct Runtime {
  status: Vec<CameraStatus>,
  gpio_pins: Option<GpioPins>,
}

#[derive(Debug)]
struct GpioPins {
    switch: InputPin,
    check_low: bool,
    led: OutputPin,
}

impl GpioPins {
    fn can_record(&mut self) -> bool {
        if (self.check_low && self.switch.is_low())
          || (!self.check_low && self.switch.is_high()) {
            self.set_led(true);
            return true;
        } else {
            self.set_led(false);
            return false;
        }
    }

    fn set_led(&mut self, on: bool) {
        if on {
            self.led.set_high();
        } else {
            self.led.set_low();
        }
    }
}


fn main() {
    let config = load_config();
    println!("Config loaded: {:#?}", config);
    let mut gpio_pins = None;
    let switch_dir = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGUSR1, Arc::clone(&switch_dir)).unwrap();

    if config.gpio_switch.is_some() {
        let gpio_result = Gpio::new();
        match gpio_result {
            Ok(g) => {
                println!("GPIO initialized");
                gpio_pins = Some(
                    GpioPins {
                        switch: g.get(config.gpio_switch.unwrap()).unwrap().into_input_pullup(),
                        led: g.get(config.gpio_led.unwrap()).unwrap().into_output(),
                        check_low: true,
                    }
                );
            },
            Err(_) => { }
        }
    }

    let mut runtime = Runtime {
        status: vec![],
        gpio_pins: gpio_pins,
    };


    config.cameras.iter().for_each(|camera| {
        runtime.status.push(CameraStatus {
          last_run: SystemTime::UNIX_EPOCH,
          config: camera.clone(),
        });
    });

    let min_interval = config.endpoints.iter().map(|e| e.interval).min().unwrap();

    loop {
        let now = SystemTime::now();
        match runtime.gpio_pins {
            Some(ref mut pins) => {
                if switch_dir.swap(false, Ordering::Relaxed) {
                    println!("Switching switch direction");
                    pins.check_low = !pins.check_low;
                }
                if! pins.can_record() {
                    sleep(Duration::new(1,0));
                    continue;
                }
            },
            None => {}
        }

        for camera in &mut runtime.status {
            let since = now.duration_since(camera.last_run).unwrap().as_secs();
            if since > min_interval {
                let image = camera.grab_image();
                if image.len() > 0 {
                    // Send image to endpoint that requires it based on interval.
                    for endpoint in &config.endpoints {
                        if since > endpoint.interval {
                            update_info(&camera.config, &endpoint);
                            send_image(&camera.config, &image, &endpoint);
                        }
                    }
                } else {
                    println!("Error grabbing image from camera {}.", camera.config.device);
                }
            }
        }
        sleep(Duration::new(1,0));
    }

}

fn load_config() -> Config {
    let f = File::open("config.yml").unwrap();
    serde_yaml::from_reader(f).unwrap()
}

fn send_image(camera: &Camera, image: &Vec<u8>, endpoint: &Endpoint) {
    let url = endpoint.snapshot_url.as_str();
    let res = ureq::put(url)
        .set("Content-Type", "image/jpg")
        .set("Accept", "*/*")
        .set("Content-Length", image.len().to_string().as_str())
        .set("Token", camera.token.as_str())
        .set("Fingerprint", camera.fingerprint.as_str())
        .send_bytes(image.as_slice());
    match res {
        Ok(_) => {
            println!("Image sent successfully to endpoint {}.", endpoint.name);
        },
        Err(e) => {
            println!("Error sending image to endpoint {}: {:?}", endpoint.name, e);
        }
    }
}

fn update_info(camera: &Camera, endpoint: &Endpoint) {
    if endpoint.info_url.is_none() {
        return;
    }

    let url = endpoint.info_url.as_ref().unwrap().as_str();
    let res = ureq::put(url)
        .set("Content-Type", "application/json")
        .set("Token", camera.token.as_str())
        .set("Fingerprint", camera.fingerprint.as_str())
        .send_json(ureq::json!(
            {
                "config": {
                  "path": camera.device.as_str(),
                  "name": camera.name.as_str(),
                  "driver": "V4L2",
                  "trigger_scheme": "THIRTY_SEC",
                  "resolution": {
                    "width": camera.resolutionx,
                    "height": camera.resolutiony
                  }
                }
              }
        ));
    match res {
        Ok(_) => {
            println!("Update info successfully to endpoint {}.", endpoint.name);
        },
        Err(e) => {
            println!("Error sending info to endpoint {}: {:?}", endpoint.name, e);
        }
    }
}
