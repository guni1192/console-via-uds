use std::ffi::CString;
use std::fs;
use std::io::prelude::*;
use std::io::{stderr, stdin, stdout};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use nix::fcntl::{open, OFlag};
use nix::pty::{grantpt, posix_openpt, ptsname_r, unlockpt};
use nix::sys::select::{select, FdSet};
use nix::sys::stat::Mode;
use nix::unistd::{close, daemon, dup3, execv, fork, read, setsid, write, ForkResult};

fn main() -> anyhow::Result<()> {
    let console_sock = PathBuf::from("console.sock");

    if console_sock.exists() {
        fs::remove_file(&console_sock)?;
    }

    let pty_master = posix_openpt(OFlag::O_RDWR)?;
    grantpt(&pty_master)?;
    unlockpt(&pty_master)?;
    let slave_name = &ptsname_r(&pty_master)?;
    println!("slave_name: {}", slave_name);
    let master_fd = pty_master.as_raw_fd();

    daemon(true, false)?;

    let listener = UnixListener::bind(&console_sock)?;
    let (mut stream, _sockaddr) = listener.accept()?;
    let stream_fd = stream.as_raw_fd();

    match fork()? {
        ForkResult::Parent { child: _ } => {
            let logfile = open(
                "pty.log",
                OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC,
                Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP,
            )?;

            loop {
                let mut infds = FdSet::new();
                infds.clear();
                infds.insert(stream_fd);
                infds.insert(master_fd);

                select(stream_fd + 1, Some(&mut infds), None, None, None)?;

                if infds.contains(stream_fd) {
                    let mut buf = [0 as u8; 4096];
                    stream.read_exact(&mut buf)?;
                    // read(stream_fd, &mut buf)?;
                    write(master_fd, &buf)?;
                }

                if infds.contains(master_fd) {
                    let mut buf = [0 as u8; 4096];
                    let size = read(master_fd, &mut buf)?;
                    if size == 0 {
                        std::process::exit(0);
                    }
                    stream.write_all(&buf)?;
                    // write(stream_fd, &buf)?;
                    write(logfile, &buf)?;
                }
            }
        }
        ForkResult::Child => {
            setsid()?;

            let path = PathBuf::from(&slave_name);
            let fds = open(&path, OFlag::O_RDWR, Mode::empty())?;

            unsafe {
                if libc::ioctl(fds, libc::TIOCSCTTY, 0) != 0 {
                    libc::perror(b"ioctl".as_ptr() as *const i8);
                    std::process::exit(1);
                }
            }

            dup3(fds, stdin().as_raw_fd(), OFlag::empty())?;
            dup3(fds, stdout().as_raw_fd(), OFlag::empty())?;
            dup3(fds, stderr().as_raw_fd(), OFlag::empty())?;
            close(fds)?;

            let bash = CString::new("/bin/bash")?;
            let args = vec![bash.as_c_str()];

            execv(&args[0], &args)?;

            Ok(())
        }
    }
}
