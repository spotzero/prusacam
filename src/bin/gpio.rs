use rppal::gpio::Gpio;

fn main() {
    let gpio = Gpio::new().unwrap();
    let mut pin = gpio.get(17).unwrap().into_input_pullup();
    print!("In: {:?}, Got: {:?}", pin.is_low(), pin.read());
}