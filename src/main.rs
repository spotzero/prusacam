use rscam;
use serde::{Serialize, Deserialize};
use std::fs::File;
use ureq;
use std::io::Cursor;
use std::io::Write;
use std::time::{Duration, SystemTime};
use std::thread::sleep;
use rppal::gpio::Gpio;

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
    interval: usize,
    gpio_switch: Option<u8>,
    gpio_led: Option<u8>,
}

#[derive(Debug)]
pub struct CameraStatus {
    last_run: SystemTime,
    status: bool,
    config: Camera,
}

impl CameraStatus {
    fn grab_image(&mut self) -> Vec<u8> {
        print!("Grabbing image from camera {}", self.config.device);
        let mut camera = rscam::Camera::new(self.config.device.as_str()).unwrap();

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
  use_gpio: bool,
}

fn main() {
    let config = load_config();

    let mut use_gpio = false;
    if config.gpio_switch.is_some() {
        use_gpio = true;
    }

    let mut runtime = Runtime {
        status: vec![],
        use_gpio: use_gpio,
    };


    config.cameras.iter().for_each(|camera| {
        runtime.status.push(CameraStatus {
          last_run: SystemTime::UNIX_EPOCH,
          status: true,
          config: camera.clone(),
        });
    });

    loop {
        let now = SystemTime::now();
        if runtime.use_gpio {
            let gpio = Gpio::new().unwrap();
            let mut pin = gpio.get(config.gpio_switch.unwrap()).unwrap().into_input_pullup();
            if pin.is_high() {
                sleep(Duration::new(1,0));
                continue;
            }
        }

        for camera in &mut runtime.status {
            if now.duration_since(camera.last_run).unwrap().as_secs() > config.interval as u64 {
                let image = camera.grab_image();
                if image.len() > 0 {
                    update_info(&camera.config);
                    send_image(&camera.config, image);
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

fn send_image(camera: &Camera, image: Vec<u8>) {
    let url = "https://connect.prusa3d.com/c/snapshot";
    let res = ureq::put(url)
        .set("Content-Type", "image/jpg")
        .set("Accept", "*/*")
        .set("Content-Length", image.len().to_string().as_str())
        .set("Token", camera.token.as_str())
        .set("Fingerprint", camera.fingerprint.as_str())
        .send_bytes(image.as_slice());
    match res {
        Ok(_) => {
            println!("Image sent successfully");
        },
        Err(e) => {
            println!("Error sending image: {:?}", e);
        }
    }
}

fn update_info(camera: &Camera) {
    let url = "https://connect.prusa3d.com/c/info";
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
            println!("Update info successfully");
        },
        Err(e) => {
            println!("Error sending info: {:?}", e);
        }
    }
}
