#![no_std]
#![no_main]

use panic_halt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::Pull;
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::pio::{Direction as PioDirection, FifoJoin, Pio, ShiftDirection};
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config};
use pio::pio_asm;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<USB>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<'static, embassy_rp::usb::Driver<'static, USB>>,
) -> ! {
    usb.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut pio = Pio::new(p.PIO0, Irqs);

    let mut pin0 = pio.common.make_pio_pin(p.PIN_0);
    let mut pin1 = pio.common.make_pio_pin(p.PIN_1);
    let mut pin2 = pio.common.make_pio_pin(p.PIN_2);
    let mut pin3 = pio.common.make_pio_pin(p.PIN_3);
    let mut pin4 = pio.common.make_pio_pin(p.PIN_4);
    let mut pin5 = pio.common.make_pio_pin(p.PIN_5);
    let mut pin6 = pio.common.make_pio_pin(p.PIN_6);
    let mut pin7 = pio.common.make_pio_pin(p.PIN_7);

    pin0.set_pull(Pull::Down);
    pin1.set_pull(Pull::Down);
    pin2.set_pull(Pull::Down);
    pin3.set_pull(Pull::Down);
    pin4.set_pull(Pull::Down);
    pin5.set_pull(Pull::Down);
    pin6.set_pull(Pull::Down);
    pin7.set_pull(Pull::Down);

    let post_prg = pio_asm!(
        r#"
                mov isr, null
                in pins, 8
                in null 24
                mov y, isr
                push noblock

            .wrap_target
            loop:
                mov isr, null
                in pins, 8
                in null 24
                mov x, isr
                jmp x!=y, changed
                jmp loop

            changed:
                mov y, x
                mov isr, x
                push noblock
                jmp loop
            .wrap
        "#
    );

    let loaded = pio.common.load_program(&post_prg.program);
    let mut sm = pio.sm0;

    let in_pins = [&pin0, &pin1, &pin2, &pin3, &pin4, &pin5, &pin6, &pin7];

    let mut pio_cfg = embassy_rp::pio::Config::default();
    pio_cfg.use_program(&loaded, &[]);
    pio_cfg.set_in_pins(&in_pins);
    pio_cfg.shift_in.auto_fill = false;
    pio_cfg.shift_in.direction = ShiftDirection::Right;
    pio_cfg.shift_in.threshold = 32;
    pio_cfg.fifo_join = FifoJoin::RxOnly;
    pio_cfg.clock_divider = 8u8.into();

    sm.set_pin_dirs(PioDirection::In, &in_pins);
    sm.clear_fifos();
    sm.set_config(&pio_cfg);
    sm.set_enable(true);

    let driver = embassy_rp::usb::Driver::new(p.USB, Irqs);

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CTRL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut config = Config::new(0xCafe, 0x4001);
    config.manufacturer = Some("picopostreader");
    config.product = Some("Pi Pico POST reader");
    config.serial_number = Some("0001");
    config.max_power = 100;

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        MSOS_DESC.init([0; 256]),
        CTRL_BUF.init([0; 64]),
    );

    static CDC_STATE: StaticCell<State<'static>> = StaticCell::new();
    let mut cdc = CdcAcmClass::new(&mut builder, CDC_STATE.init(State::new()), 64);

    let usb = builder.build();

    let _ = spawner.spawn(usb_task(usb));

    let rx = sm.rx();
    loop {
        cdc.wait_connection().await;

        let mut last_sent = 0xFFu8;
        loop {
            let mut latest: Option<u8> = None;

            while let Some(word) = rx.try_pull() {
                latest = Some((word & 0xFF) as u8);
            }

            let post = match latest {
                Some(v) => v,
                None => (rx.wait_pull().await & 0xFF) as u8,
            };

            if post == last_sent {
                continue;
            }

            match with_timeout(Duration::from_millis(20), cdc.write_packet(&[post])).await {
                Ok(Ok(())) => last_sent = post,
                Ok(Err(_)) => break,
                Err(_) => {}
            }
        }
    }
}
