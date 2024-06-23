# Prusacam

This is a simple daemon that periodically grabs images from local webcams and uploads them to Prusa Connect.

If running on a Raspberry PI, it support the connection of a switch/toggle and LED and the camera uploading can be enabled with the switch (and the LED is turned on when uploading images and off when disabled),

## Features

Support taking periodic captures from multiple webcams and uploading them to Prusa Connect.

Supports the connection of a switch or toggle the the Raspberry Pi GPIO to act and an enable/disable of camera uploading.

Supports connection of an LED on the Raspberry Pi GPIO to at as an indicator of whether images are being uploaded.

Supports switching the camera on and off with signals.

Currently only supports Linux.


## Getting Started

### Building

Install rust and run: 

```
cargo build --bin prusacam
```

To build a binary for the old Raspberry Pi's, install "cross" with:

```
cargo install cross
```

And build with:

```
cross build --target arm-unknown-linux-gnueabihf -r --bin prusacam
```

### GPIO Setup

The GPIO pin are configurable, but it worked for me with GPIO17 and GPIO18.

Warning: Messing around with the GPIO pin can fry your pi, and I'm not responsible for that.

#### Switch

Connect one off of the switch or toggle to GPIO17 and the other to ground.

GPIO17 will be set to pull-up, so when it's shorted to ground, prusacam will capture and upload images.

#### LED

Connect the positive side of the LED to GPIO18, and the negative leg to a 330 ohm resistor, and then the resistor to ground.

### Configuration

The must be a config.yml file in the current working directory for prusacam to start.  This config files control all the cameras and keys:

config.yml should look like this:
```
gpio_switch: 17      # GPIO input pin for the switch/toggle.
gpio_led: 18         # GPIO output pin for the LED.
cameras:             # An array of all the cameras, any number are supported and they'll be processed one at a time.
  -
    name: "My camera 0"           # Camera name (what will appear in Prusa Connect)
    device: /dev/video0           # Camera device
    token: Camera 0's token       # Camera's prusa connect token
    fingerprint: Fingerprint - between 16 and 64 characters  # Camera's fingerprint, just set it to some random characters (but at least 16 of them)
    resolutionx: 1920             # The width of the image you want to capture (must be supported by the camera)
    resolutiony: 1080             # The height of the image you want to capture (must be supported by the camera)
endpoints:
  -
    name: "Prusa Connect"                                        # Name of endpoint set
    interval: 30                                                 # Number of seconds between uploading.
    snapshot_url: https://connect.prusa3d.com/c/snapshot         # Snapshot PUT URL.
    info_url: https://connect.prusa3d.com/c/info                 # Info URL (Optional)
```

### Installing

The prusacam binary can just be executed as is.

### Executing program

```
nohup ./prusacam &
```

### Switching the camera on and off from the command line

`prusacam` will listen for the signal USR1 to act as the camera toggle.

Just run: `killall prusacam -USR1` or `kill $PID -USR1`

When a physical switch is connected and setup, both switches act like a three-way switch.

## Authors

David Pascoe-Deslauriers [@spotzero](https://github.com/spotzero)

## License

This software is licensed under the Apache License 2.0, see LICENSE.txt
