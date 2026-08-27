use std::fmt;
use std::io::prelude::*;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use socket2::{Domain, SockAddr, Socket, Type};

pub struct EmacsClient {
    socket_path: PathBuf,
    timeout: Duration,
}

#[derive(Debug)]
pub enum EmacsError {
    Io(std::io::Error),
    Eval(String),
    Protocol(String),
}

impl std::error::Error for EmacsError {}

impl fmt::Display for EmacsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EmacsError::Io(err) => err.fmt(f),
            EmacsError::Eval(msg) => write!(f, "eval error: {}", msg),
            EmacsError::Protocol(msg) => write!(f, "protocol error: {}", msg),
        }
    }
}

impl From<std::io::Error> for EmacsError {
    fn from(err: std::io::Error) -> Self {
        EmacsError::Io(err)
    }
}

impl EmacsClient {
    pub fn new(socket_path: &Path, timeout: Duration) -> EmacsClient {
        EmacsClient {
            socket_path: socket_path.to_owned(),
            timeout,
        }
    }

    pub fn eval(&mut self, expr: &str) -> Result<String, EmacsError> {
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        let address = SockAddr::unix(&self.socket_path)?;
        socket.connect_timeout(&address, self.timeout)?;
        let fd: OwnedFd = socket.into();
        let mut sock = UnixStream::from(fd);
        sock.set_read_timeout(Some(self.timeout))?;
        sock.set_write_timeout(Some(self.timeout))?;

        let cmd = format!("-current-frame -eval {} \n", Self::quote_argument(expr));
        sock.write_all(cmd.as_bytes())?;
        sock.flush()?;

        let mut response = String::new();
        sock.read_to_string(&mut response)?;

        for line in response.lines() {
            if let Some(value) = line.strip_prefix("-print ") {
                return Self::unquote_argument(value);
            } else if let Some(value) = line.strip_prefix("-error ") {
                return Err(EmacsError::Eval(Self::unquote_argument(value)?));
            }
        }

        Err(EmacsError::Protocol(format!(
            "expected '-print' or '-error' while evaluating {:?}, got {:?}",
            expr, response
        )))
    }

    /// Quote an argument to the Emacs server.
    ///
    /// Inserts a '&' before each '&', each space, each newline and any initial '-'.
    /// Changes space to underscores, too, so that the return value never contains a
    /// space.
    ///
    /// https://github.com/emacs-mirror/emacs/blob/cde5dcd441b5db79f39b8664221866566c400b05/lib-src/emacsclient.c#L828
    fn quote_argument(s: &str) -> String {
        let mut chars = s.chars();
        let start = match chars.next() {
            Some('-') => "&-".to_owned(),
            Some(ch) => ch.to_string(),
            None => "".to_owned(),
        };

        start
            + &chars
                .flat_map(|c| match c {
                    ' ' => vec!['&', '_'],
                    '\n' => vec!['&', 'n'],
                    '&' => vec!['&', '&'],
                    _ => vec![c],
                })
                .collect::<String>()
    }

    /// Unquote an argument from the Emacs server.
    fn unquote_argument(s: &str) -> Result<String, EmacsError> {
        let mut chars = s.chars();
        let mut out = String::new();
        while let Some(ch) = chars.next() {
            if ch == '&' {
                match chars.next() {
                    Some('_') => out.push(' '),
                    Some('n') => out.push('\n'),
                    Some(ch) => out.push(ch),
                    None => {
                        return Err(EmacsError::Protocol(format!(
                            "truncated escape sequence in server argument {:?}",
                            s
                        )));
                    }
                }
            } else {
                out.push(ch);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Instant;

    static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn test_quote_argument() {
        assert_eq!(
            EmacsClient::quote_argument("(message \"test\")"),
            "(message&_\"test\")"
        );
    }

    #[test]
    fn test_unquote_argument() {
        assert_eq!(
            EmacsClient::unquote_argument("(a&_+&_1)&n").unwrap(),
            "(a + 1)\n"
        );
        assert!(matches!(
            EmacsClient::unquote_argument("broken&"),
            Err(EmacsError::Protocol(_))
        ));
    }

    #[test]
    fn eval_sends_emacs_server_protocol_and_parses_print() {
        let (socket_path, server) = mock_server("-print t\n");
        let mut client = EmacsClient::new(&socket_path, Duration::from_millis(250));

        assert_eq!(client.eval("(message \"hello world\")").unwrap(), "t");
        assert_eq!(
            server.join().unwrap(),
            "-current-frame -eval (message&_\"hello&_world\") \n"
        );
        let _ = fs::remove_file(socket_path);
    }

    #[test]
    fn silent_server_is_bounded_by_read_timeout() {
        let socket_path = test_socket_path();
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(250));
        });
        let mut client = EmacsClient::new(&socket_path, Duration::from_millis(30));
        let started = Instant::now();
        assert!(matches!(client.eval("t"), Err(EmacsError::Io(_))));
        assert!(started.elapsed() < Duration::from_millis(200));
        server.join().unwrap();
        let _ = fs::remove_file(socket_path);
    }

    #[test]
    fn eval_decodes_server_error_without_panicking() {
        let (socket_path, server) = mock_server("-error bad&_command\n");
        let mut client = EmacsClient::new(&socket_path, Duration::from_millis(250));

        let error = client.eval("(+ 1 2)").unwrap_err();
        assert!(matches!(error, EmacsError::Eval(ref msg) if msg == "bad command"));
        server.join().unwrap();
        let _ = fs::remove_file(socket_path);
    }

    #[test]
    fn malformed_server_response_is_a_protocol_error() {
        let (socket_path, server) = mock_server("unexpected response\n");
        let mut client = EmacsClient::new(&socket_path, Duration::from_millis(250));

        let error = client.eval("t").unwrap_err();
        assert!(matches!(error, EmacsError::Protocol(_)));
        server.join().unwrap();
        let _ = fs::remove_file(socket_path);
    }

    fn mock_server(response: &'static str) -> (PathBuf, thread::JoinHandle<String>) {
        let socket_path = test_socket_path();
        let listener = UnixListener::bind(&socket_path).unwrap();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut request)
                .unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            request
        });

        (socket_path, server)
    }

    fn test_socket_path() -> PathBuf {
        let socket_path = std::env::temp_dir().join(format!(
            "emacs-i3-test-{}-{}.sock",
            std::process::id(),
            SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&socket_path);
        socket_path
    }
}
