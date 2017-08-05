use core::fmt;

const MAX_PATH: usize = 128;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum State {
    WaitG,
    WaitE,
    WaitT,
    WaitSpace,
    GetPath,
    WaitCR1,
    WaitLF1,
    WaitCR2,
    WaitLF2,
    Finished
}

pub struct Request {
    state: State,
    path_idx: usize,
    path: [u8; MAX_PATH]
}

impl Request {
    pub fn new() -> Request {
        Request {
            state: State::WaitG,
            path_idx: 0,
            path: [0; MAX_PATH]
        }
    }

    pub fn reset(&mut self) {
        self.state = State::WaitG;
        self.path_idx = 0;
    }
    
    pub fn input_char(&mut self, c: u8) -> Result<bool, &'static str> {
        match self.state {
            State::WaitG => {
                if c == b'G' {
                    self.state = State::WaitE;
                } else {
                    return Err("invalid character in method")
                }
            }
            State::WaitE => {
                if c == b'E' {
                    self.state = State::WaitT;
                } else {
                    return Err("invalid character in method")
                }
            }
            State::WaitT => {
                if c == b'T' {
                    self.state = State::WaitSpace;
                } else {
                    return Err("invalid character in method")
                }
            }
            State::WaitSpace => {
                if c == b' ' {
                    self.state = State::GetPath;
                } else {
                    return Err("invalid character in method")
                }
            }
            State::GetPath => {
                if c == b'\r' || c == b'\n' {
                    return Err("GET ended prematurely")
                } else if c == b' ' {
                    if self.path_idx == 0 {
                        return Err("path is empty")
                    } else {
                        self.state = State::WaitCR1;
                    }
                } else {
                    if self.path_idx >= self.path.len() {
                        return Err("path is too long")
                    } else {
                        self.path[self.path_idx] = c;
                        self.path_idx += 1;
                    }
                }
            }
            State::WaitCR1 => {
                if c == b'\r' {
                    self.state = State::WaitLF1;
                }
            }
            State::WaitLF1 => {
                if c == b'\n' {
                    self.state = State::WaitCR2;
                } else {
                    self.state = State::WaitCR1;
                }
            }
            State::WaitCR2 => {
                if c == b'\r' {
                    self.state = State::WaitLF2;
                } else {
                    self.state = State::WaitCR1;
                }
            }
            State::WaitLF2 => {
                if c == b'\n' {
                    self.state = State::Finished;
                    return Ok(true)
                } else {
                    self.state = State::WaitCR1;
                }
            }
            State::Finished => return Err("trailing characters")
        }
        Ok(false)
    }
    
    pub fn input(&mut self, buf: &[u8]) -> Result<bool, &'static str> {
        let mut result = Ok(false);
        for c in buf.iter() {
            result = self.input_char(*c);
            if result.is_err() {
                return result;
            }
        }
        result
    }
    
    pub fn get_path<'a>(&'a self) -> Result<&'a [u8], &'static str> {
        if self.state != State::Finished {
            return Err("request is not finished")
        }
        Ok(&self.path[..self.path_idx])
    }
}

pub fn write_reply_header(output: &mut fmt::Write, status: u16, content_type: &str, gzip: bool) -> fmt::Result {
    let status_text = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => return Err(fmt::Error)
    };
    write!(output, "HTTP/1.1 {} {}\r\nContent-Type: {}\r\n",
           status, status_text, content_type)?;
    if gzip {
        write!(output, "Content-Encoding: gzip\r\n")?;
    }
    write!(output, "\r\n")
}
