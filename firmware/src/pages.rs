use core::fmt::Write;
use smoltcp::socket::TcpSocket;
use http;

pub fn serve(output: &mut TcpSocket, request: &http::Request) {
    match request.get_path().unwrap() {
        b"/" => {
            let data = include_str!("index.html");
            http::write_reply_header(output, 200, "text/html; charset=utf-8", false).unwrap();
            output.write_str(data).unwrap();
        },
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
