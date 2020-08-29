use std::io::prelude::*;
use std::io::stdin;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

use nix::sys::select::{select, FdSet};
use nix::unistd::read;

fn main() -> anyhow::Result<()> {
    let mut stream = UnixStream::connect("console.sock")?;

    let stream_fd = stream.as_raw_fd();
    let stdin_fd = stdin().as_raw_fd();

    loop {
        let mut infds = FdSet::new();
        infds.clear();
        infds.insert(stream_fd);
        infds.insert(stdin_fd);

        select(stream_fd + 1, Some(&mut infds), None, None, None)?;

        // stream -> stdout
        if infds.contains(stream_fd) {
            let mut buf = [0 as u8; 4096];
            let n = stream.read(&mut buf)?;
            let line = String::from_utf8_lossy(&buf[..n]);
            print!("{}", line);
        }

        // stdin -> stream
        if infds.contains(stdin_fd) {
            let mut buf = [0; 4096];
            read(stdin_fd, &mut buf)?;
            stream.write_all(&buf)?;
            // return self input
            stream.read_exact(&mut buf)?;
        }
    }
}
