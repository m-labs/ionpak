#![feature(used, const_fn, core_float, asm, lang_items, conservative_impl_trait)]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_rt;
#[macro_use(interrupt)]
extern crate tm4c129x;
extern crate smoltcp;

use core::cell::{Cell, RefCell};
use core::fmt;
use cortex_m::interrupt::Mutex;
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;
use smoltcp::iface::{NeighborCache, EthernetInterfaceBuilder};
use smoltcp::socket::{SocketSet, TcpSocket, TcpSocketBuffer};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        write!($crate::UART0, $($arg)*).unwrap()
    })
}

#[macro_export]
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

#[no_mangle]
#[lang = "panic_fmt"]
pub fn panic_fmt(msg: fmt::Arguments, file: &'static str, line: u32, col: u32) -> ! {
    println!("panic at {}:{}:{}: {}", file, line, col, msg);
    loop {}
}

#[macro_use]
mod board;
mod eeprom;
mod crc32;
mod config;
mod ethmac;
mod pid;
mod loop_anode;
mod loop_cathode;
mod electrometer;
mod http;
mod pages;

static ADC_IRQ_COUNT: Mutex<Cell<u64>> = Mutex::new(Cell::new(0));

fn get_time_ms() -> u64 {
    let adc_irq_count = cortex_m::interrupt::free(|cs| {
        ADC_IRQ_COUNT.borrow(cs).get()
    });
    adc_irq_count*24/125
}

static LOOP_ANODE: Mutex<RefCell<loop_anode::Controller>> = Mutex::new(RefCell::new(
    loop_anode::Controller::new()));

static LOOP_CATHODE: Mutex<RefCell<loop_cathode::Controller>> = Mutex::new(RefCell::new(
    loop_cathode::Controller::new()));

static ELECTROMETER: Mutex<RefCell<electrometer::Electrometer>> = Mutex::new(RefCell::new(
    electrometer::Electrometer::new()));


pub struct UART0;

impl fmt::Write for UART0 {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for c in s.bytes() {
            unsafe {
                let uart_0 = tm4c129x::UART0.get();
                while (*uart_0).fr.read().txff().bit() {}
                (*uart_0).dr.write(|w| w.data().bits(c))
            }
        }
        Ok(())
    }
}

const TCP_RX_BUFFER_SIZE: usize = 256;
const TCP_TX_BUFFER_SIZE: usize = 8192;


macro_rules! create_socket_storage {
    ($rx_storage:ident, $tx_storage:ident) => (
        let mut $rx_storage = [0; TCP_RX_BUFFER_SIZE];
        let mut $tx_storage = [0; TCP_TX_BUFFER_SIZE];
    )
}

macro_rules! create_socket {
    ($set:ident, $rx_storage:ident, $tx_storage:ident, $target:ident) => (
        let tcp_rx_buffer = TcpSocketBuffer::new(&mut $rx_storage[..]);
        let tcp_tx_buffer = TcpSocketBuffer::new(&mut $tx_storage[..]);
        let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);
        let $target = $set.add(tcp_socket);
    )
}

fn main() {
    board::init();
    let button_pressed = board::get_button();

    let mut config = config::Config::new();
    eeprom::init();
    if button_pressed {
        config.save();
    } else {
        config.load();
    }

    cortex_m::interrupt::free(|cs| {
        let mut loop_anode = LOOP_ANODE.borrow(cs).borrow_mut();
        let mut loop_cathode = LOOP_CATHODE.borrow(cs).borrow_mut();

        // ZJ-10
        let anode = 165.0;
        let cathode_bias = 50.0;
        let emission = 0.5e-3;

        // ZJ-27
        /*let anode = 225.0;
        let cathode_bias = 25.0;
        let emission = 1.0e-3;*/

        // ZJ-12
        /*let anode = 200.0;
        let cathode_bias = 50.0;
        let emission = 4.0e-3;*/

        // G8130
        /*let anode = 180.0;
        let cathode_bias = 30.0;
        let emission = 4.0e-3;*/

        loop_anode.set_target(anode);
        loop_cathode.set_emission_target(emission);
        loop_cathode.set_bias_target(cathode_bias);
    });

    println!(r#"
  _                         _
 (_)                       | |
  _  ___  _ __  _ __   __ _| |
 | |/ _ \| '_ \| '_ \ / _` | |/ /
 | | (_) | | | | |_) | (_| |   <
 |_|\___/|_| |_| .__/ \__,_|_|\_\
               | |
               |_|
"#);

    let mut hardware_addr = EthernetAddress(board::get_mac_address());
    if hardware_addr.is_multicast() {
        println!("programmed MAC address is invalid, using default");
        hardware_addr = EthernetAddress([0x10, 0xE2, 0xD5, 0x00, 0x03, 0x00]);
    }
    let mut ip_addrs = [config.ip];
    println!("MAC {} IP {}", hardware_addr, ip_addrs[0]);
    let mut neighbor_cache_storage = [None; 8];
    let neighbor_cache = NeighborCache::new(&mut neighbor_cache_storage[..]);
    let mut device = ethmac::Device::new();
    unsafe { device.init(hardware_addr) };
    let mut iface = EthernetInterfaceBuilder::new(&mut device)
                .ethernet_addr(hardware_addr)
                .neighbor_cache(neighbor_cache)
                .ip_addrs(&mut ip_addrs[..])
                .finalize();

    create_socket_storage!(tcp_rx_storage0, tcp_tx_storage0);
    create_socket_storage!(tcp_rx_storage1, tcp_tx_storage1);
    create_socket_storage!(tcp_rx_storage2, tcp_tx_storage2);
    create_socket_storage!(tcp_rx_storage3, tcp_tx_storage3);
    create_socket_storage!(tcp_rx_storage4, tcp_tx_storage4);
    create_socket_storage!(tcp_rx_storage5, tcp_tx_storage5);
    create_socket_storage!(tcp_rx_storage6, tcp_tx_storage6);
    create_socket_storage!(tcp_rx_storage7, tcp_tx_storage7);

    let mut socket_set_entries: [_; 8] = Default::default();
    let mut sockets = SocketSet::new(&mut socket_set_entries[..]);

    create_socket!(sockets, tcp_rx_storage0, tcp_tx_storage0, tcp_handle0);
    create_socket!(sockets, tcp_rx_storage1, tcp_tx_storage1, tcp_handle1);
    create_socket!(sockets, tcp_rx_storage2, tcp_tx_storage2, tcp_handle2);
    create_socket!(sockets, tcp_rx_storage3, tcp_tx_storage3, tcp_handle3);
    create_socket!(sockets, tcp_rx_storage4, tcp_tx_storage4, tcp_handle4);
    create_socket!(sockets, tcp_rx_storage5, tcp_tx_storage5, tcp_handle5);
    create_socket!(sockets, tcp_rx_storage6, tcp_tx_storage6, tcp_handle6);
    create_socket!(sockets, tcp_rx_storage7, tcp_tx_storage7, tcp_handle7);

    let mut sessions = [
        (http::Request::new(), tcp_handle0),
        (http::Request::new(), tcp_handle1),
        (http::Request::new(), tcp_handle2),
        (http::Request::new(), tcp_handle3),
        (http::Request::new(), tcp_handle4),
        (http::Request::new(), tcp_handle5),
        (http::Request::new(), tcp_handle6),
        (http::Request::new(), tcp_handle7),
    ];

    board::start_adc();

    let mut fast_blink_count = if button_pressed { 40 } else { 0 };
    let mut next_blink = 0;
    let mut led_state = true;
    let mut latch_reset_time = None;
    loop {
        let time = get_time_ms();

        for &mut(ref mut request, tcp_handle) in sessions.iter_mut() {
            let socket = &mut *sockets.get::<TcpSocket>(tcp_handle);
            if !socket.is_open() {
                socket.listen(80).unwrap()
            }

            if socket.may_recv() {
                match socket.recv(|data| (data.len(), request.input(data))).unwrap() {
                    Ok(true) => {
                        if socket.can_send() {
                            pages::serve(socket, &request, &mut config, &LOOP_ANODE, &LOOP_CATHODE, &ELECTROMETER);
                        }
                        request.reset();
                        socket.close();
                    }
                    Ok(false) => (),
                    Err(err) => {
                        println!("failed HTTP request: {}", err);
                        request.reset();
                        socket.close();
                    }
                }
            } else if socket.may_send() {
                request.reset();
                socket.close();
            }
        }
        match iface.poll(&mut sockets, Instant::from_millis(time as i64)) {
            Ok(_) => (),
            Err(e) => println!("poll error: {}", e)
        }

        if time > next_blink {
            led_state = !led_state;
            if fast_blink_count > 0 {
                fast_blink_count -= 1;
                next_blink = time + 100;
            } else {
                next_blink = time + 500;
            }
            board::set_led(led_state);
        }

        board::process_errors();
        if board::error_latched() {
            match latch_reset_time {
                None => {
                    println!("Protection latched");
                    latch_reset_time = Some(time + 5000);
                }
                Some(t) => if time > t {
                    latch_reset_time = None;
                    cortex_m::interrupt::free(|cs| {
                        // reset PID loops as they have accumulated large errors
                        // while the protection was active, which would cause
                        // unnecessary overshoots.
                        LOOP_ANODE.borrow(cs).borrow_mut().reset();
                        LOOP_CATHODE.borrow(cs).borrow_mut().reset();
                        board::reset_error();
                    });
                    println!("Protection reset");
                }
            }
        }
    }
}

interrupt!(ADC0SS0, adc0_ss0);
fn adc0_ss0() {
    cortex_m::interrupt::free(|cs| {
        let adc0 = tm4c129x::ADC0.borrow(cs);
        if adc0.ostat.read().ov0().bit() {
            panic!("ADC FIFO overflowed")
        }
        adc0.isc.write(|w| w.in0().bit(true));

        let ic_sample  = adc0.ssfifo0.read().data().bits();
        let fbi_sample = adc0.ssfifo0.read().data().bits();
        let fv_sample  = adc0.ssfifo0.read().data().bits();
        let fd_sample  = adc0.ssfifo0.read().data().bits();
        let av_sample  = adc0.ssfifo0.read().data().bits();
        let fbv_sample = adc0.ssfifo0.read().data().bits();

        let mut loop_anode = LOOP_ANODE.borrow(cs).borrow_mut();
        let mut loop_cathode = LOOP_CATHODE.borrow(cs).borrow_mut();
        let mut electrometer = ELECTROMETER.borrow(cs).borrow_mut();
        loop_anode.adc_input(av_sample);
        loop_cathode.adc_input(fbi_sample, fd_sample, fv_sample, fbv_sample);
        electrometer.adc_input(ic_sample);

        let adc_irq_count = ADC_IRQ_COUNT.borrow(cs);
        adc_irq_count.set(adc_irq_count.get() + 1);
    });
}
