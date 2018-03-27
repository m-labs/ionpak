use core::fmt;

const MAX_QUERY: usize = 128;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum State {
    WaitG,
    WaitE,
    WaitT,
    WaitSpace,
    GetQuery,
    WaitCR1,
    WaitLF1,
    WaitCR2,
    WaitLF2,
    Finished
}

pub struct Request {
    state: State,
    query_idx: usize,
    query: [u8; MAX_QUERY]
}

impl Request {
    pub fn new() -> Request {
        Request {
            state: State::WaitG,
            query_idx: 0,
            query: [0; MAX_QUERY]
        }
    }

    pub fn reset(&mut self) {
        self.state = State::WaitG;
        self.query_idx = 0;
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
                    self.state = State::GetQuery;
                } else {
                    return Err("invalid character in method")
                }
            }
            State::GetQuery => {
                if c == b'\r' || c == b'\n' {
                    return Err("GET ended prematurely")
                } else if c == b' ' {
                    if self.query_idx == 0 {
                        return Err("query is empty")
                    } else {
                        self.state = State::WaitCR1;
                    }
                } else {
                    if self.query_idx >= self.query.len() {
                        return Err("query is too long")
                    } else {
                        self.query[self.query_idx] = c;
                        self.query_idx += 1;
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

    pub fn get_query<'a>(&'a self) -> Result<&'a [u8], &'static str> {
        if self.state != State::Finished {
            return Err("request is not finished")
        }
        Ok(&self.query[..self.query_idx])
    }

    pub fn get_path<'a>(&'a self) -> Result<&'a [u8], &'static str> {
        let query = self.get_query()?;
        Ok(query.split(|b| *b == '?' as u8).next().unwrap())
    }

    // FIXME: this yields some empty strings
    pub fn iter_args<'a>(&'a self) -> Result<impl Iterator<Item=(&'a [u8], &'a [u8])>, &'static str> {
        let query = self.get_query()?;
        let mut qs = query.split(|b| *b == '?' as u8);
        qs.next();
        let args = qs.next().unwrap_or(b"");
        let args_it = args.split(|b| *b == '&' as u8);
        Ok(args_it.map(|arg| {
            let mut eqs = arg.split(|b| *b == '=' as u8);
            (eqs.next().unwrap(), eqs.next().unwrap_or(b""))
        }))
    }

    pub fn get_arg<'a>(&'a self, name: &[u8]) -> Result<&'a [u8], &'static str> {
        for (current_name, current_value) in self.iter_args()? {
            if current_name == name {
                return Ok(current_value)
            }
        }
        Err("argument not found")
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
        write!(output, "Cache-Control: public, max-age=600\r\n")?;
    }
    write!(output, "\r\n")
}
