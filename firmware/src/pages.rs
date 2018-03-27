use core::fmt;
use core::fmt::Write;
use core::cell::RefCell;
use core::str;
use cortex_m;
use cortex_m::interrupt::Mutex;
use smoltcp::wire::IpCidr;
use smoltcp::socket::TcpSocket;

use http;
use config;
use loop_anode;
use loop_cathode;
use electrometer;

macro_rules! opn_fmt {
    ($struct_name:ident, $error:expr) => {
        struct $struct_name(Option<f32>);

        impl fmt::Display for $struct_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.0 {
                    None => f.write_str($error),
                    Some(x) => x.fmt(f)
                }
            }
        }

        impl fmt::LowerExp for $struct_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.0 {
                    None => f.write_str($error),
                    Some(x) => x.fmt(f)
                }
            }
        }
    }
}

opn_fmt!(OpnFmt, "ERROR");
opn_fmt!(OpnFmtJSON, "null");

pub fn serve(output: &mut TcpSocket, request: &http::Request,
             config: &mut config::Config,
             loop_anode_m: &Mutex<RefCell<loop_anode::Controller>>,
             loop_cathode_m: &Mutex<RefCell<loop_cathode::Controller>>,
             electrometer_m: &Mutex<RefCell<electrometer::Electrometer>>) {
    match request.get_path().unwrap() {
        b"/" => {
            let (anode, cathode, electrometer) = cortex_m::interrupt::free(|cs| {
                (loop_anode_m.borrow(cs).borrow().get_status(),
                 loop_cathode_m.borrow(cs).borrow().get_status(),
                 electrometer_m.borrow(cs).borrow().get_status())
            });

            let pressure = electrometer.ic.and_then(|ic| {
                if ic > 1.0e-12 {
                    cathode.fbi.and_then(|fbi| Some(ic/fbi/18.75154))
                } else {
                    None
                }
            });
            http::write_reply_header(output, 200, "text/html; charset=utf-8", false).unwrap();
            write!(output, include_str!("index.html"),
                pressure=OpnFmt(pressure),
                anode_ready=anode.ready,
                anode_av=OpnFmt(anode.av),
                cathode_ready=cathode.ready,
                cathode_fbi=OpnFmt(cathode.fbi.and_then(|x| Some(x*1.0e6))),
                cathode_fv=OpnFmt(cathode.fv),
                cathode_fv_target=OpnFmt(cathode.fv_target),
                cathode_fbv=OpnFmt(cathode.fbv),
                ion_current=OpnFmt(electrometer.ic.and_then(|x| Some(x*1.0e9)))).unwrap();
        },
        b"/measure.json" => {
            let (cathode, electrometer) = cortex_m::interrupt::free(|cs| {
                (loop_cathode_m.borrow(cs).borrow().get_status(),
                 electrometer_m.borrow(cs).borrow().get_status())
            });

            // TODO: factor this
            let pressure = electrometer.ic.and_then(|ic| {
                if ic > 1.0e-12 {
                    cathode.fbi.and_then(|fbi| Some(ic/fbi/18.75154))
                } else {
                    None
                }
            });
            http::write_reply_header(output, 200, "application/json", false).unwrap();
            write!(output, "{{\"pressure\": {:.1e}, \"current\": {:.3e}}}",
                   OpnFmtJSON(pressure), OpnFmtJSON(electrometer.ic)).unwrap();
        }
        b"/network_settings.html" => {
            let mut status = "";

            let ip_arg = request.get_arg(b"ip");
            if ip_arg.is_ok() {
                let ip_arg = str::from_utf8(ip_arg.unwrap());
                if ip_arg.is_ok() {
                    let mut ip_arg = ip_arg.unwrap().split("%2F");
                    let ip = ip_arg.next().map(|x| x.parse());
                    let cidr = ip_arg.next().map(|x| x.parse());
                    match (ip, cidr) {
                        (Some(Ok(ip)), Some(Ok(cidr))) => {
                            status = "IP address has been updated and will be active after a reboot.";
                            config.ip = IpCidr::new(ip, cidr);
                            config.save();
                        }
                        _ =>
                            status = "failed to parse IP address"
                    }
                } else {
                    status = "IP address contains an invalid UTF-8 character";
                }
            }

            http::write_reply_header(output, 200, "text/html; charset=utf-8", false).unwrap();
            write!(output, include_str!("network_settings.html"),
                   status=status, ip=config.ip).unwrap();
        },
        b"/firmware.html" => {
            http::write_reply_header(output, 200, "text/html; charset=utf-8", false).unwrap();
            write!(output, include_str!("firmware.html"),
                   version=include_str!(concat!(env!("OUT_DIR"), "/git-describe"))).unwrap();
        }
        b"/style.css" => {
            let data = include_bytes!("style.css.gz");
            http::write_reply_header(output, 200, "text/css", true).unwrap();
            output.send_slice(data).unwrap();
        },
        b"/logo.svg" => {
            let data = include_bytes!("logo.svg.gz");
            http::write_reply_header(output, 200, "image/svg+xml", true).unwrap();
            output.send_slice(data).unwrap();
        },
        _ => {
            http::write_reply_header(output, 404, "text/plain", false).unwrap();
            write!(output, "Not found").unwrap();
        }
    }
}
