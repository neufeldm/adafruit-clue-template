#![no_main]
#![no_std]

// Sample app that reads a bunch of Adafruit Clue sensors
// and displays them on the screen. Modify as needed.

use adafruit_clue::{Board, TFT};
use cortex_m_rt;
use embedded_hal::blocking::delay::DelayMs;
use nrf52840_hal::{Delay, Timer};
// accelerometer/Gyro
use lsm6ds33::Lsm6ds33;
use lsm6ds33::{AccelerometerBandwidth, AccelerometerOutput, AccelerometerScale};
use lsm6ds33::{GyroscopeFullScale, GyroscopeOutput};

// proximity/gesture/color
use apds9960::Apds9960;

// pressure/temperature
use bmp280_rs;

// magnetometer
use lis3mdl;

// humidity
use sht3x;

use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::*;
use embedded_graphics::{
    mono_font::{ascii::FONT_7X13, MonoTextStyle},
    text::Text,
};
use shared_bus;
use st7789::{Orientation, ST7789};

use core::fmt::Write;
use heapless::String;

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut b = Board::take().unwrap();
    let sensor_i2c = b.sensors_i2c.twim(b.TWIM1);
    let shared_sensor_i2c = shared_bus::BusManagerSimple::new(sensor_i2c);
    let mut gyro_accel =
        Lsm6ds33::new(shared_sensor_i2c.acquire_i2c(), Board::I2C_GYROACCEL).unwrap();
    gyro_accel
        .set_accelerometer_scale(AccelerometerScale::G02)
        .unwrap();
    gyro_accel
        .set_accelerometer_bandwidth(AccelerometerBandwidth::Freq100)
        .unwrap();
    gyro_accel
        .set_accelerometer_output(AccelerometerOutput::Rate104)
        .unwrap();
    gyro_accel
        .set_gyroscope_scale(GyroscopeFullScale::Dps245)
        .unwrap();
    gyro_accel
        .set_gyroscope_output(GyroscopeOutput::Rate104)
        .unwrap();
    gyro_accel.set_low_power_mode(false).unwrap();

    let mut prox_rgb_gesture = Apds9960::new(shared_sensor_i2c.acquire_i2c());
    prox_rgb_gesture.enable().unwrap();
    prox_rgb_gesture.enable_light().unwrap();

    let temp_pressure_config = bmp280_rs::Config {
        measurement_standby_time_millis: Some(
            bmp280_rs::MeasurementStandbyTimeMillis::ZeroPointFive,
        ),
        pressure_oversampling: bmp280_rs::PressureOversampling::Four,
        temperature_oversampling: bmp280_rs::TemperatureOversampling::Four,
        iir_filter: bmp280_rs::IIRFilterCoefficient::Four,
    };
    let mut temp_pressure_i2c = shared_sensor_i2c.acquire_i2c();
    let temp_pressure_sleep = bmp280_rs::BMP280::new(
        &mut temp_pressure_i2c,
        bmp280_rs::I2CAddress::SdoPulledUp,
        temp_pressure_config,
    )
    .unwrap();
    let mut temp_pressure = temp_pressure_sleep
        .into_normal_mode(&mut temp_pressure_i2c)
        .unwrap();

    let magnet_i2c = shared_sensor_i2c.acquire_i2c();
    let mut magnetometer = lis3mdl::Lis3mdl::new(magnet_i2c, lis3mdl::Address::Addr1C).unwrap();

    let humidity_i2c = shared_sensor_i2c.acquire_i2c();
    let mut humidity = sht3x::SHT3x::new(humidity_i2c, sht3x::Address::Low);

    // TFT SPI
    b.tft.backlight_on();
    let tft_display_interface =
        SPIInterfaceNoCS::new(b.tft.spim(b.SPIM0), b.tft.dc.take().unwrap());
    let mut display = ST7789::new(
        tft_display_interface,
        b.tft.reset.take().unwrap(),
        TFT::XSIZE,
        TFT::YSIZE,
    );
    let mut delay = Delay::new(b.core_peripherals.SYST);
    display.init(&mut delay).unwrap();
    display.set_orientation(Orientation::Landscape).unwrap();
    display.clear(Rgb565::BLACK).unwrap();
    let mut timer = Timer::new(b.TIMER4);

    let rgb_circle = |x, y, size, color: Rgb565| {
        Circle::new(Point::new(x, y), size).into_styled(PrimitiveStyle::with_fill(color))
    };

    let clear_text_rect = |x, y| {
        Rectangle::new(Point::new(x, y), Size::new(240, 20))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    };

    loop {
        // gyro/accel
        let (x, y, z) = gyro_accel.read_gyro().unwrap();
        let mut gyrostring: String<64> = String::new();
        write!(gyrostring, "GYRO ({:.4},{:.4},{:.4})", x, y, z).unwrap();
        clear_text_rect(0, 220).draw(&mut display).unwrap();
        text(0, 220, &gyrostring).draw(&mut display).unwrap();

        // prox/rgb/gesture
        let rgb = prox_rgb_gesture.read_light().unwrap();
        let circle_color = Rgb565::new(rgb.red as u8, rgb.green as u8, rgb.blue as u8);
        let mut rgbstring: String<64> = String::new();
        write!(
            rgbstring,
            "RGB ({},{},{},{})",
            rgb.red, rgb.green, rgb.blue, rgb.clear
        )
        .unwrap();
        clear_text_rect(0, 200).draw(&mut display).unwrap();
        rgb_circle(10, 200, 10, circle_color)
            .draw(&mut display)
            .unwrap();
        text(20, 200, &rgbstring).draw(&mut display).unwrap();

        // temp/pressure
        let temp = temp_pressure
            .read_temperature(&mut temp_pressure_i2c)
            .unwrap();
        let pressure = temp_pressure.read_pressure(&mut temp_pressure_i2c).unwrap();
        let mut tempstring: String<64> = String::new();
        write!(tempstring, "TEMP {}", temp).unwrap();
        clear_text_rect(0, 180).draw(&mut display).unwrap();
        text(0, 180, &tempstring).draw(&mut display).unwrap();
        let mut pressstring: String<64> = String::new();
        write!(pressstring, "PRESSURE {}", pressure).unwrap();
        clear_text_rect(0, 160).draw(&mut display).unwrap();
        text(0, 160, &pressstring).draw(&mut display).unwrap();

        // magnetometer
        let xyz = magnetometer.get_mag_axes_mgauss().unwrap();
        let mut magstring: String<64> = String::new();
        write!(magstring, "MAG ({},{},{})", xyz.x, xyz.y, xyz.z).unwrap();
        clear_text_rect(0, 140).draw(&mut display).unwrap();
        text(0, 140, &magstring).draw(&mut display).unwrap();

        // humidity
        let h = humidity
            .measure(sht3x::Repeatability::High, &mut delay)
            .unwrap();
        let mut humstring: String<64> = String::new();
        write!(humstring, "HUMID {} TEMP {}", h.humidity, h.temperature).unwrap();
        clear_text_rect(0, 120).draw(&mut display).unwrap();
        text(0, 120, &humstring).draw(&mut display).unwrap();

        timer.delay_ms(250 as u32);
    }
}

fn text(x: i32, y: i32, s: &str) -> Text<MonoTextStyle<Rgb565>> {
    let text_style = MonoTextStyle::new(&FONT_7X13, Rgb565::WHITE);
    Text::new(s, Point::new(x + 10, y + 10), text_style)
}

#[panic_handler] // panicking behavior
unsafe fn panic(_pinfo: &core::panic::PanicInfo) -> ! {
    let mut b: Board = Board::steal();
    let mut timer = Timer::new(b.TIMER3);
    loop {
        b.leds.white.on();
        timer.delay_ms(500 as u32);
        b.leds.white.off();
        timer.delay_ms(100 as u32);
        //cortex_m::asm::bkpt();
    }
}
