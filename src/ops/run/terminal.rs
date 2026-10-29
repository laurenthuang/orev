use super::redact::UsernameRedactor;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(25);
const DRAIN_IDLE: Duration = Duration::from_millis(250);
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(2);

pub(super) fn execute(
    argv: &[OsString],
    executable: &Path,
    cwd: &Path,
    log: &mut File,
    username: &[u8],
    signal: &AtomicI32,
) -> io::Result<u32> {
    if argv.is_empty() || username.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a command and a nonempty username are required",
        ));
    }
    let size = terminal_size();
    let pair = native_pty_system().openpty(size).map_err(pty_error)?;
    let mut command = CommandBuilder::new(executable);
    command.args(&argv[1..]);
    command.cwd(cwd);
    let _raw = RawTerminal::new()?;
    let child = pair.slave.spawn_command(command).map_err(pty_error)?;
    drop(pair.slave);
    let mut child = RunningChild {
        child,
        reaped: false,
        foreground: None,
    };
    let mut redactor = UsernameRedactor::new(username);
    let result = relay(
        pair.master.as_ref(),
        &mut child,
        log,
        &mut redactor,
        signal,
        size,
    );
    // Flush even on relay errors, so a partially matched final component is
    // never silently lost from an otherwise valid log.
    let finish = redactor.finish(log);
    result.and_then(|code| finish.map(|()| code))
}

fn pty_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn terminal_size() -> PtySize {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn resize(master: &dyn MasterPty, previous: &mut PtySize) -> io::Result<()> {
    let size = terminal_size();
    if size != *previous {
        master.resize(size).map_err(pty_error)?;
        *previous = size;
    }
    Ok(())
}

struct RawTerminal(bool);

impl RawTerminal {
    fn new() -> io::Result<Self> {
        let terminal = io::stdin().is_terminal() && !crossterm::terminal::is_raw_mode_enabled()?;
        if terminal {
            crossterm::terminal::enable_raw_mode()?;
        }
        Ok(Self(terminal))
    }
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        if self.0 {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

struct RunningChild {
    child: Box<dyn Child + Send + Sync>,
    reaped: bool,
    foreground: Option<i32>,
}

impl RunningChild {
    fn poll(&mut self) -> io::Result<Option<u32>> {
        let Some(status) = self.child.try_wait()? else {
            return Ok(None);
        };
        self.reaped = true;
        if let Some(name) = status.signal() {
            for number in 1..=64 {
                let pointer = unsafe { libc::strsignal(number) };
                if !pointer.is_null()
                    && unsafe { std::ffi::CStr::from_ptr(pointer) }.to_bytes() == name.as_bytes()
                {
                    return Ok(Some(128 + number as u32));
                }
            }
        }
        Ok(Some(status.exit_code()))
    }

    fn terminate(&mut self) {
        self.send_signal(libc::SIGKILL);
        let _ = self.child.kill();
    }

    fn send_signal(&self, signal: i32) {
        let leader = self.child.process_id().map(|pid| pid as i32);
        for group in [self.foreground, leader].into_iter().flatten() {
            if group > 0 {
                unsafe { libc::kill(-group, signal) };
            }
        }
        if let Some(pid) = leader {
            unsafe { libc::kill(pid, signal) };
        }
    }
}

impl Drop for RunningChild {
    fn drop(&mut self) {
        if !self.reaped {
            self.terminate();
            let _ = self.child.wait();
        }
    }
}

fn relay(
    master: &dyn MasterPty,
    child: &mut RunningChild,
    log: &mut File,
    redactor: &mut UsernameRedactor,
    signal: &AtomicI32,
    mut size: PtySize,
) -> io::Result<u32> {
    let fd = master
        .as_raw_fd()
        .ok_or_else(|| io::Error::other("PTY has no native descriptor"))?;
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut stdout = io::stdout().lock();
    let mut input = true;
    let mut pending = VecDeque::<u8>::new();
    let mut status = None;
    let mut exited = None;
    let mut last_output = Instant::now();
    let mut forwarded = 0;
    let mut stopping = None;
    let mut output_open = true;
    loop {
        child.foreground = master.process_group_leader();
        if status.is_none()
            && let Some(code) = child.poll()?
        {
            status = Some(code);
            exited = Some(Instant::now());
            input = false;
            pending.clear();
        }
        let requested = signal.load(Ordering::Relaxed);
        if requested != 0 && requested != forwarded && status.is_none() {
            child.send_signal(requested);
            forwarded = requested;
            stopping.get_or_insert_with(Instant::now);
        }
        if stopping.is_some_and(|at: Instant| at.elapsed() >= SHUTDOWN_LIMIT) && status.is_none() {
            child.terminate();
        }
        if let Some(code) = status
            && (!output_open
                || exited.is_some_and(|at: Instant| {
                    at.elapsed() >= SHUTDOWN_LIMIT
                        || (at.elapsed() >= DRAIN_IDLE && last_output.elapsed() >= DRAIN_IDLE)
                }))
        {
            stdout.flush()?;
            return Ok(code);
        }
        resize(master, &mut size)?;
        let mut descriptors = [
            libc::pollfd {
                fd,
                events: (if output_open { libc::POLLIN } else { 0 })
                    | if pending.is_empty() { 0 } else { libc::POLLOUT },
                revents: 0,
            },
            libc::pollfd {
                fd: if input && pending.len() < 65536 {
                    libc::STDIN_FILENO
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let ready = unsafe { libc::poll(descriptors.as_mut_ptr(), 2, TICK.as_millis() as i32) };
        if ready == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if output_open
            && descriptors[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
        {
            let mut buffer = [0; 16384];
            // Bound each batch so output floods cannot starve signals/input.
            for _ in 0..16 {
                let count = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
                if count > 0 {
                    let bytes = &buffer[..count as usize];
                    redactor.write(bytes, log)?;
                    stdout.write_all(bytes)?;
                    stdout.flush()?;
                    last_output = Instant::now();
                } else if count == 0 {
                    output_open = false;
                    break;
                } else {
                    let error = io::Error::last_os_error();
                    if error.raw_os_error() == Some(libc::EIO) {
                        output_open = false;
                        break;
                    }
                    if error.kind() == io::ErrorKind::WouldBlock {
                        break;
                    }
                    if error.kind() != io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                }
            }
        }
        if descriptors[1].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            let mut buffer = [0; 4096];
            let count =
                unsafe { libc::read(libc::STDIN_FILENO, buffer.as_mut_ptr().cast(), buffer.len()) };
            if count > 0 {
                pending.extend(&buffer[..count as usize]);
            } else if count == 0 {
                input = false;
                let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
                if unsafe { libc::tcgetattr(fd, &mut termios) } == 0
                    && termios.c_lflag & libc::ICANON != 0
                {
                    let eof = termios.c_cc[libc::VEOF];
                    if eof != 0 {
                        // The first EOF flushes an unterminated line; the
                        // second makes the next canonical read return EOF.
                        pending.extend([eof, eof]);
                    }
                }
            } else {
                let error = io::Error::last_os_error();
                if !matches!(
                    error.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) {
                    return Err(error);
                }
            }
        }
        if !pending.is_empty() && output_open {
            let bytes = pending.make_contiguous();
            let count = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
            if count > 0 {
                pending.drain(..count as usize);
            } else if count == -1 {
                let error = io::Error::last_os_error();
                if !matches!(
                    error.kind(),
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
                ) && error.raw_os_error() != Some(libc::EIO)
                {
                    return Err(error);
                }
            }
        }
    }
}
